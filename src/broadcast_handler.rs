use std::sync::Arc;
use tokio::net::UdpSocket as TokioUdpSocket;
use tokio::sync::mpsc;

use crate::data_types::MatchResult;
use crate::message_codec; // 引入 Codec

pub struct BroadcastHandler {
    socket: Arc<TokioUdpSocket>,
    multicast_addr: String,
}

impl BroadcastHandler {
    pub fn new(socket: Arc<TokioUdpSocket>, multicast_addr: String) -> Self {
        BroadcastHandler {
            socket,
            multicast_addr,
        }
    }

    pub async fn start_broadcasting(
        &self,
        mut rx: mpsc::Receiver<MatchResult>,
    ) {
        while let Some(result) = rx.recv().await {
            // 序列化成交结果
            let message = message_codec::serialize_match_result(&result);

            // 广播成交信息
            if let Err(e) = self.socket.send_to(&message, &self.multicast_addr).await {
                eprintln!("[BROADCAST] Failed to send trade broadcast: {}", e);
            } else {
                println!("[BROADCAST] Sent trade result: {:?}", result);
            }
        }
        println!("[BROADCAST] Broadcast handler stopped.");
    }
}
