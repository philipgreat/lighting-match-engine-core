use crate::data_types::{BroadcastStats, EngineState, MESSAGE_TOTAL_SIZE};
use crate::message_codec;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tokio::time::{self, Duration};

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLockReadGuard, RwLockWriteGuard};

use crate::data_types::OrderBook;

//use data_types::OrderBook;

impl EngineState {
    /// Creates a new EngineState instance with initialized components.
    pub fn new(instance_tag: [u8; 8], product_id: u16, status_multicast_addr: SocketAddr) -> Self {
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fail")
            .as_nanos() as u64;

        EngineState {
            instance_tag,
            product_id,
            order_book: Arc::new(RwLock::new(OrderBook::new(10))),
            matched_orders: Arc::new(RwLock::new(0)),
            total_received_orders: Arc::new(RwLock::new(0)),
            start_time: now_nanos,
            status_multicast_addr,
        }
    }

    pub async fn get_order_book_to_write(&self) -> RwLockWriteGuard<'_, OrderBook> {
        // 调用 .write().await 等待获取独占写入锁
        self.order_book.write().await
    }
    pub async fn get_order_book_to_read(&self) -> RwLockReadGuard<'_, OrderBook> {
        // 调用 .write().await 等待获取独占写入锁
        self.order_book.read().await
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
            let order_book = self.state.order_book.read().await;
            let matched_orders = self.state.matched_orders.read().await;
            let total_received_orders = self.state.total_received_orders.read().await;

            // 2. Construct the stats message
            let stats = BroadcastStats {
                instance_tag: self.state.instance_tag,
                product_id: self.state.product_id,
                order_book_size: order_book.len() as u32,
                matched_orders: *matched_orders as u32,
                total_received_orders: *total_received_orders as u32,
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
