use crate::data_types::{BroadcastStats, CallAuctionPool, EngineState, MESSAGE_TOTAL_SIZE};
use crate::message_codec;

use crate::data_types::ContinuousOrderBook;
// use crate::data_types::CallAuctionPool;

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
            continuous_order_book: ContinuousOrderBook::new(1000, 100),
            call_auction_pool: CallAuctionPool::new(1000),
            matched_orders: 0,
            total_received_orders:0 ,
            start_time: now_nanos,
        }
    }

    /// Creates a self-contained handler for status broadcasting logic.
    pub fn new_status_broadcaster(
        state: Arc<EngineState>
    ) -> StatusBroadcaster {
        StatusBroadcaster { state }
    }

    pub  fn increase_match(&mut self) {
        self.matched_orders  = self.matched_orders + 1;
        
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
