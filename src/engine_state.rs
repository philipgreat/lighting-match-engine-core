use crate::data_types::{BroadcastStats, EngineState, MESSAGE_TOTAL_SIZE};
use crate::message_codec;

use tokio::net::UdpSocket;
use tokio::time::{self, Duration};

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

impl EngineState {
    /// Creates a new EngineState instance with initialized components.
    pub fn new(
        instance_tag: [u8; 8],
        product_id: u16,
        trade_multicast_addr: SocketAddr,
        status_multicast_addr: SocketAddr,
    ) -> Self {
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fail")
            .as_nanos() as u64;

        EngineState {
            instance_tag,
            product_id,
            order_book: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            matched_orders: Arc::new(tokio::sync::Mutex::new(0)),
            total_received_orders: Arc::new(tokio::sync::Mutex::new(0)),
            start_time: now_nanos,
            trade_multicast_addr,
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
            let order_book = self.state.order_book.lock().await;
            let matched_orders = self.state.matched_orders.lock().await;
            let total_received_orders = self.state.total_received_orders.lock().await;

            // 2. Construct the stats message
            let stats = BroadcastStats {
                instance_tag: self.state.instance_tag,
                product_id: self.state.product_id,
                order_book_size: order_book.len() as u64,
                matched_orders: *matched_orders,
                total_received_orders: *total_received_orders,
                start_time: self.state.start_time,
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
