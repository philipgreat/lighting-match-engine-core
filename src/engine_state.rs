use std::sync::Arc;
use std::time::{SystemTime, Duration};
use tokio::net::UdpSocket as TokioUdpSocket;

use crate::data_types::*;
use crate::message_codec; 

impl EngineState {
    pub fn new(product_id: u16, broadcast_socket: Arc<TokioUdpSocket>, multicast_addr: String) -> Self {
        let now_nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_nanos(0))
            .as_nanos() as u64;

        EngineState {
            product_id,
            order_book: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            matched_orders: Arc::new(tokio::sync::Mutex::new(0)),
            total_received_orders: Arc::new(tokio::sync::Mutex::new(0)),
            start_time: now_nanos,
            broadcast_socket,
            multicast_addr,
        }
    }
    
    // 广播引擎状态统计信息
    pub async fn broadcast_stats(&self) {
        // 必须先获取订单簿的锁来获取当前大小
        let book_guard = self.order_book.lock().await;
        let order_book_size = book_guard.len() as u64;
        // 释放 book_guard，以便其他任务可以访问
        drop(book_guard); 

        let matched_count_guard = self.matched_orders.lock().await;
        let received_count_guard = self.total_received_orders.lock().await;
        
        let stats = BroadcastStats {
            product_id: self.product_id,
            order_book_size, // New field
            matched_orders: *matched_count_guard,
            total_received_orders: *received_count_guard,
            start_time: self.start_time,
        };

        let message = message_codec::serialize_stats_result(&stats);

        if let Err(e) = self.broadcast_socket.send_to(&message, &self.multicast_addr).await {
            eprintln!("[STATE] Failed to broadcast stats: {}", e);
        } else {
            println!("[STATE] Broadcasted stats: Book Size={}, Matched={}, Received={}", 
                stats.order_book_size, stats.matched_orders, stats.total_received_orders);
        }
    }
}

// impl Clone for EngineState {
//     fn clone(&self) -> Self {
//         EngineState {
//             product_id: self.product_id,
//             order_book: Arc::clone(&self.order_book),
//             matched_orders: Arc::clone(&self.matched_orders),
//             total_received_orders: Arc::clone(&self.total_received_orders),
//             start_time: self.start_time,
//             broadcast_socket: Arc::clone(&self.broadcast_socket),
//             multicast_addr: self.multicast_addr.clone(),
//         }
//     }
// }
