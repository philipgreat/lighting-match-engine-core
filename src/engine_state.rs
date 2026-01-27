use crate::data_types::{BroadcastStats, EngineState, MESSAGE_TOTAL_SIZE};
use crate::message_codec;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tokio::time::{self, Duration};
use crate::data_types::OrderBook;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
impl EngineState {
    /// Creates a new EngineState instance with initialized components.
    pub fn new(instance_tag: [u8; 16], product_id: u16, status_multicast_addr: SocketAddr) -> Self {
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fail")
            .as_nanos() as u64;

        EngineState {
            instance_tag,
            product_id,
            order_book: Arc::new(RwLock::new(OrderBook::new(10000, 100))),
            matched_orders: Arc::new(RwLock::new(0)),
            total_received_orders: Arc::new(RwLock::new(0)),
            start_time: now_nanos,
            status_multicast_addr,
        }
    }

    /// Creates a self-contained handler for status broadcasting logic.
    pub fn new_status_broadcaster(
        state: Arc<EngineState>,
        socket: Arc<UdpSocket>,
    ) -> StatusBroadcaster {
        StatusBroadcaster { state, socket }
    }

    pub async fn increase_match(&self) {
        let mut match_count = self.matched_orders.write().await;
        *match_count += 1;
    }
}

/// Handler responsible for periodically broadcasting the engine's current state/stats.
pub struct StatusBroadcaster {
    state: Arc<EngineState>,
    socket: Arc<UdpSocket>,
}

impl StatusBroadcaster {
    /// Runs the periodic status broadcast loop.
    pub async fn run_status_broadcast(&self) {
        let mut interval = time::interval(Duration::from_secs(10));

        let addr = self.state.status_multicast_addr;
        println!("Status broadcaster started. Target address: {}", addr);

        loop {
            // Wait for the next tick
            interval.tick().await;

            // 1. Lock necessary shared data
            let order_book = self.state.order_book.read().await;
            let matched_orders = self.state.matched_orders.read().await;
            let total_received_orders = self.state.total_received_orders.read().await;

            // 2. Construct the stats message
            let stats = BroadcastStats {
                instance_tag: self.state.instance_tag,
                product_id: self.state.product_id,
                bids_order_count: order_book.bids.len() as u32,
                ask_order_count: order_book.asks.len() as u32,
                matched_orders: *matched_orders as u32,
                total_received_orders: *total_received_orders as u32,
                start_time: self.state.start_time,
                total_bid_volumn: order_book.total_bid_volumn,
                total_ask_volumn: order_book.total_ask_volumn,
            };
            println!("status info {:?}", stats);
            
            // 3. Serialize and send
            let buf: [u8; MESSAGE_TOTAL_SIZE] = message_codec::serialize_stats_result(&stats);
            if let Err(e) = self.socket.send_to(&buf, addr).await {
                eprintln!("Error sending status broadcast: {}", e);
            }
        }
    }
}
