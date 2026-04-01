use alloc::collections::VecDeque;

#[derive(Clone, Copy)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Clone, Copy)]
pub struct DemoOrder {
    pub order_id: u64,
    pub side: Side,
    pub price: u64,
    pub quantity: u32,
}

impl DemoOrder {
    pub const fn new(order_id: u64, side: Side, price: u64, quantity: u32) -> Self {
        Self {
            order_id,
            side,
            price,
            quantity,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Trade {
    pub buy_order_id: u64,
    pub sell_order_id: u64,
    pub price: u64,
    pub quantity: u32,
}

pub struct DemoOrderBook {
    asks: VecDeque<DemoOrder>,
}

impl DemoOrderBook {
    pub fn new() -> Self {
        Self {
            asks: VecDeque::new(),
        }
    }

    pub fn seed(&mut self, first: DemoOrder, second: DemoOrder) {
        self.asks.push_back(first);
        self.asks.push_back(second);
    }

    pub fn match_order(&mut self, mut order: DemoOrder) -> Option<Trade> {
        if !matches!(order.side, Side::Buy) {
            return None;
        }

        while let Some(resting) = self.asks.front_mut() {
            if order.price < resting.price {
                return None;
            }

            let matched = order.quantity.min(resting.quantity);
            order.quantity -= matched;
            resting.quantity -= matched;

            let trade = Trade {
                buy_order_id: order.order_id,
                sell_order_id: resting.order_id,
                price: resting.price,
                quantity: matched,
            };

            if resting.quantity == 0 {
                self.asks.pop_front();
            }

            return Some(trade);
        }

        None
    }
}
