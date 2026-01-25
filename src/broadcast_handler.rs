use crate::data_types::{MESSAGE_TOTAL_SIZE, MatchResult};
use crate::message_codec;

use tokio::net::UdpSocket;
use tokio::sync::mpsc::Receiver;

use std::net::SocketAddr;
use std::sync::Arc;

/// Handler responsible for sending out matched trade results.
pub struct TradeNetworkTime {
    socket: Arc<UdpSocket>,
    trade_multicast_addr: SocketAddr,
    receiver: Receiver<MatchResult>,
}

impl TradeNetworkTime {
    /// Creates a new trade_network_time.
    pub fn new(
        socket: Arc<UdpSocket>,
        trade_multicast_addr: SocketAddr,
        receiver: Receiver<MatchResult>,
    ) -> Self {
        TradeNetworkTime {
            socket,
            trade_multicast_addr,
            receiver,
        }
    }

    /// Runs the main loop to listen for Trades and broadcast them.
    pub async fn run_broadcast_loop(&mut self) {
        println!(
            "Trade broadcaster started. Target address: {}",
            self.trade_multicast_addr
        );
        while let Some(result) = self.receiver.recv().await {
            // Serialize the Trade into the fixed 50-byte buffer
            //
            let chunks = message_codec::serialize_match_result(&result);

            for buf in chunks {
                if let Err(e) = self
                    .socket
                    .send_to(&buf, self.trade_multicast_addr)
                    .await
                {
                    eprintln!("Error sending trade broadcast: {}", e);
                }
            }
        }
    }
}
