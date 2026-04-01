use crate::date_time_tool::SystemClock;
use crate::data_types::{EngineState, ORDER_PRICE_TYPE_LIMIT, ORDER_TYPE_BUY, ORDER_TYPE_SELL, Order};
use crate::engine_core::clock::Clock;


impl EngineState {
    pub fn new(instance_tag: [u8; 16], product_id: u16) -> Self {
        Self::new_with_clock(instance_tag, product_id, SystemClock)
    }

    pub fn new_with_clock<C: Clock>(instance_tag: [u8; 16], product_id: u16, clock: C) -> Self {
        Self::new_with_start_time(instance_tag, product_id, clock.now_ns())
    }

    pub fn load_sample_test_book(&mut self, test_order_book_size: u32) {
        for i in 0..test_order_book_size {
            let order = self.create_buy_order(i);
            self.order_book.fuel_order(order);
        }
        for i in 0..test_order_book_size {
            let order = self.create_sell_order(i, test_order_book_size);
            self.order_book.fuel_order(order);
        }

    }

    pub fn create_buy_order(&self, index: u32) -> Order {
        self.create_buy_order_with_clock(index, SystemClock)
    }

    pub fn create_buy_order_with_clock<C: Clock>(&self, index: u32, clock: C) -> Order {
        let time_now = clock.now_ns();
        Order {
            product_id: self.product_id,
            order_id: (index + 1) as u64,
            order_side: ORDER_TYPE_BUY,
            price_type: ORDER_PRICE_TYPE_LIMIT,
            price: (index + 1) as u64,
            quantity: 2,
            submit_time: time_now,
            expire_time: time_now + 1000 * 1000 * 1000 * 1000 * 10,
            _padding: [0u8; 24]
        }
    }

    pub fn create_sell_order(&self, index: u32, size: u32) -> Order {
        self.create_sell_order_with_clock(index, size, SystemClock)
    }

    pub fn create_sell_order_with_clock<C: Clock>(&self, index: u32, size: u32, clock: C) -> Order {
        let time_now = clock.now_ns();
        Order {
            product_id: self.product_id,
            order_id: (size + index + 1) as u64,
            order_side: ORDER_TYPE_SELL,
            price_type: ORDER_PRICE_TYPE_LIMIT,
            price: (size + 1 + index) as u64,
            quantity: 2,
            submit_time: time_now,
            expire_time: time_now + 1000 * 1000 * 1000 * 1000 * 10,
            _padding: [0u8; 24]
        }
    }
}
