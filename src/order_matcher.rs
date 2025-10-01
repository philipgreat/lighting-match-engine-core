use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::mpsc;
use crate::data_types::*;


pub struct OrderMatcher {
    state: EngineState,
    match_tx: mpsc::Sender<MatchResult>, // 撮合结果广播通道
}

impl OrderMatcher {
    pub fn new(state: EngineState, match_tx: mpsc::Sender<MatchResult>) -> Self {
        OrderMatcher {
            state,
            match_tx,
        }
    }

    // 获取当前纳秒时间戳 (用于过期检查)
    fn current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::from_nanos(0))
            .as_nanos() as u64
    }

    // 核心处理循环：接收并分发所有传入的消息
    pub async fn process_orders(&self, mut rx: mpsc::Receiver<IncomingMessage>) {
        while let Some(message) = rx.recv().await {
            match message {
                IncomingMessage::Order(order) => {
                    self.handle_order_submission(order).await;
                },
                IncomingMessage::Cancel(cancel) => {
                    self.handle_order_cancellation(cancel).await;
                }
            }
        }
        println!("[MATCHER] Order processing task stopped.");
    }
    
    // 处理订单提交
    async fn handle_order_submission(&self, order: Order) {
        println!("[MATCHER] Processing order submission: {:?}", order);
        
        // 1. 移除过期订单
        self.remove_expired_orders().await;
        
        // 2. 尝试撮合 
        let mut remaining_order = Some(order);
        
        while let Some(mut current_order) = remaining_order.take() {
            if let Some(match_result) = self.match_orders(&mut current_order).await { 
                // 广播成交结果
                if let Err(e) = self.match_tx.send(match_result.clone()).await {
                    eprintln!("[MATCHER] Failed to send match result for broadcast: {}", e);
                }
                
                // 处理当前订单的部分成交 (Taker 订单)
                if current_order.quantity > 0 {
                    // 如果 quantity > 0，说明它是部分成交，需要继续撮合
                    remaining_order = Some(current_order);
                }
                // 如果 quantity == 0，说明完全成交，循环结束
                
            } else {
                // 如果无法撮合，添加到订单簿
                let is_market_order = current_order.price_type == ORDER_PRICE_TYPE_MARKET;

                if is_market_order {
                    // 市价单不能进入订单簿 
                    println!("[MATCHER] Market order ID {} not fully filled, remaining quantity {} is cancelled.", 
                        current_order.order_id, current_order.quantity);
                } else {
                    // 限价单进入订单簿
                    let mut book = self.state.order_book.lock().await;
                    book.push(current_order);
                }
                break; // 无法继续撮合，结束循环
            }
        }
    }
    
    // 处理订单撤销
    async fn handle_order_cancellation(&self, cancel: CancelOrder) {
        println!("[MATCHER] Processing order cancellation: {:?}", cancel);
        
        let mut book = self.state.order_book.lock().await;
        
        let initial_len = book.len();
        
        // 使用 Vec::retain() 高效地原地移除匹配 asset_id 和 order_id 的订单
        book.retain(|order| {
            // 如果 asset_id 和 order_id 都匹配，则返回 false (即移除它)
            if order.asset_id == cancel.asset_id && order.order_id == cancel.order_id {
                println!("[MATCHER] Order cancelled successfully: ID {}", order.order_id);
                false 
            } else {
                true // 保留元素
            }
        });
        
        let removed_count = initial_len - book.len();
        if removed_count == 0 {
            eprintln!("[MATCHER] Warning: Attempted to cancel non-existent order ID {} for asset {}", 
                cancel.order_id, cancel.asset_id);
        }
    }


    // 移除订单簿中所有已过期的订单
    async fn remove_expired_orders(&self) {
        let mut book = self.state.order_book.lock().await;
        let now = self.current_timestamp();
        
        let initial_len = book.len();

        book.retain(|order| {
            // 如果 expire_time 为 0 (GTC) 或 expire_time > now，则保留 (返回 true)
            // 否则，移除 (返回 false)
            order.expire_time == 0 || order.expire_time > now
        });

        let removed_count = initial_len - book.len();
        if removed_count > 0 {
            println!("[MATCHER] Removed {} expired orders.", removed_count);
        }
    }

    // 撮合买卖订单 (价格优先, 时间优先)
    // 接受可变引用，以便在撮合过程中更新 Taker 订单的剩余数量
    async fn match_orders(&self, new_order: &mut Order) -> Option<MatchResult> {
        let mut book = self.state.order_book.lock().await;

        let mut best_match_index: Option<usize> = None;
        let mut best_submit_time: u64 = u64::MAX; 
        
        // 确定价格搜索目标（市价单总是寻找最优价）
        let mut best_price: u64 = match new_order.order_type {
            ORDER_TYPE_BUY => u64::MAX,  // 买单寻找最低卖价
            ORDER_TYPE_SELL => 0,       // 卖单寻找最高买价
            _ => 0, // 仅支持 BUY/SELL 方向
        };


        // 1. 寻找最佳匹配订单 (价格优先, 时间优先)
        for (i, existing_order) in book.iter().enumerate() {
            if new_order.asset_id != existing_order.asset_id {
                continue;
            }

            // 检查订单方向是否相反且可撮合
            let is_match = match (new_order.order_type, existing_order.order_type) {
                // Taker Buy vs Maker Sell
                (ORDER_TYPE_BUY, ORDER_TYPE_SELL) => {
                    new_order.price_type == ORDER_PRICE_TYPE_MARKET || new_order.price >= existing_order.price
                },
                // Taker Sell vs Maker Buy
                (ORDER_TYPE_SELL, ORDER_TYPE_BUY) => {
                    new_order.price_type == ORDER_PRICE_TYPE_MARKET || new_order.price <= existing_order.price
                },
                // 同向订单不撮合
                _ => false, 
            };

            if is_match {
                // 评估价格优先级：
                // Buy Taker 寻找 Lowest Price (Maker Sell)
                // Sell Taker 寻找 Highest Price (Maker Buy)
                let is_new_best = match new_order.order_type {
                    ORDER_TYPE_BUY => existing_order.price < best_price,
                    ORDER_TYPE_SELL => existing_order.price > best_price,
                    _ => false,
                };
                
                // 如果是更优的价格，或者价格相同但时间更早（时间优先）
                let is_time_priority = existing_order.submit_time < best_submit_time;
                
                if is_new_best || (existing_order.price == best_price && is_time_priority) {
                    best_match_index = Some(i);
                    best_price = existing_order.price;
                    best_submit_time = existing_order.submit_time;
                }
            }
        }
        
        // 2. 如果找到匹配项，则执行成交
        if let Some(i) = best_match_index {
            // 移除被撮合的订单 (Maker Order)
            let mut existing_order = book.remove(i); 
            
            // 确定 Buy/Sell 方用于 MatchResult
            let (buy_order_id, sell_order_id) = match new_order.order_type {
                ORDER_TYPE_BUY => (new_order.order_id, existing_order.order_id),
                ORDER_TYPE_SELL => (existing_order.order_id, new_order.order_id),
                _ => return None,
            };
            
            let quantity = new_order.quantity.min(existing_order.quantity);
            
            // 更新成交计数器
            let mut matched_count_guard = self.state.matched_orders.lock().await;
            *matched_count_guard += 1;

            // 检查 Maker Order (existing_order) 是否部分成交
            if existing_order.quantity > quantity {
                existing_order.quantity -= quantity;
                // Order 不再是 Copy，需要显式 clone() 以避免所有权移动导致的后续错误
                book.push(existing_order.clone()); 
            }
            
            // 更新 Taker Order (new_order) 的剩余数量
            new_order.quantity -= quantity;
            
            // 返回 MatchResult
            Some(MatchResult {
                asset_id: new_order.asset_id,
                buy_order_id,
                sell_order_id,
                price: existing_order.price, // 成交价使用 Maker (existing_order) 的价格
                quantity,
            })
        } else {
            None
        }
    }
}
