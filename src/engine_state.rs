use crate::data_types::{BroadcastStats, CallAuctionPool, EngineState, MESSAGE_TOTAL_SIZE};
use crate::message_codec;

use crate::data_types::ContinuousOrderBook;
// use crate::data_types::CallAuctionPool;
use crate::data_types::{
     ORDER_PRICE_TYPE_LIMIT, ORDER_TYPE_BUY, ORDER_TYPE_SELL, Order,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};


impl EngineState {
    /// Creates a new EngineState instance with initialized components.
    pub fn new(instance_tag: [u8; 16], product_id: u16) -> Self {
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fail")
            .as_nanos() as u64;

        EngineState {
            instance_tag,
            product_id,
            //continuous_order_book: Arc::new((ContinuousOrderBook::new(10000, 100)),
            //call_auction_pool:Arc::new(CallAuctionPool::new(10000)),
            continuous_order_book: ContinuousOrderBook::new(100000, 1,1_000_000,100),
            call_auction_pool: CallAuctionPool::new(1000),
            matched_orders: 0,
            total_received_orders:0 ,
            start_time: now_nanos,
        }
    }
    
    /// Creates a self-contained handler for status broadcasting logic.

    

    pub  fn increase_match(&mut self) {
        self.matched_orders  = self.matched_orders + 1;
        
    }

    pub  fn match_order(&mut self, new_order: Order) {
        
        self.continuous_order_book.match_order(new_order);

    }

    pub  fn load_sample_test_book(&mut self, test_order_book_size:u32 ) {
        
        for i in 0..test_order_book_size {
            let order = self.create_buy_order(i);
            self.continuous_order_book.fuel_order(order);
        }
        for i in 0..test_order_book_size {
            let order = self.create_sell_order(i, test_order_book_size);
            self.continuous_order_book.fuel_order(order);
        }

    }


    
    pub fn create_buy_order(&self, index: u32) -> Order {
        //let time_now = time::Instant::now().elapsed().as_nanos() as u64;
        let time_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fail")
            .as_nanos() as u64;
        Order {
            product_id: self.product_id,
            order_id: (index + 1) as u64,
            order_type: ORDER_TYPE_BUY,
            price_type: ORDER_PRICE_TYPE_LIMIT,
            price: (index + 1) as u64,
            quantity: 2,
            submit_time: time_now,
            expire_time: time_now + 1000 * 1000 * 1000 * 1000 * 10,
        }
    }

    pub fn create_sell_order(&self, index: u32, size: u32) -> Order {
        let time_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fail")
            .as_nanos() as u64;

        Order {
            product_id: self.product_id,
            order_id: (size + index + 1) as u64,
            order_type: ORDER_TYPE_SELL,
            price_type: ORDER_PRICE_TYPE_LIMIT,
            price: (size + 1 + index) as u64,
            quantity: 2,
            submit_time: time_now,
            expire_time: time_now + 1000 * 1000 * 1000 * 1000 * 10,
        }
    }
    


}

/// Handler responsible for periodically broadcasting the engine's current state/stats.
pub struct StatusBroadcaster {
    state: Arc<EngineState>,
}

impl StatusBroadcaster {

    // pub async fn run_status_broadcast(&self) {


    //     println!("Status broadcaster started.");

    //     loop {
    //         // Wait for the next tick

    //         // 1. Lock necessary shared data
    //         let continuous_order_book = self.state.continuous_order_book;
    //         let matched_orders = self.state.matched_orders;
    //         let total_received_orders = self.state.total_received_orders;

    //         // 2. Construct the stats message
    //         let stats = BroadcastStats {
    //             instance_tag: self.state.instance_tag,
    //             product_id: self.state.product_id,
    //             bids_order_count: continuous_order_book.bids.len() as u32,
    //             ask_order_count: continuous_order_book.asks.len() as u32,
    //             matched_orders: matched_orders as u32,
    //             total_received_orders: total_received_orders as u32,
    //             start_time: self.state.start_time,
    //             total_bid_volumn: continuous_order_book.total_bid_volumn,
    //             total_ask_volumn: continuous_order_book.total_ask_volumn,
    //         };
    //         //println!("status info {:?}", stats);
            
    //         // 3. Serialize and send
    //         let buf: [u8; MESSAGE_TOTAL_SIZE] = message_codec::serialize_stats_result(&stats);
            
    //         println!("{:?}",buf)
    //     }
    // }
}
