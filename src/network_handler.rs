use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket as TokioUdpSocket;
use tokio::sync::mpsc;
use tokio::time;

use crate::data_types::*;
use crate::message_codec::{self, MESSAGE_TOTAL_SIZE}; // 引入 Codec 和统一大小

pub struct NetworkHandler {
    socket: Arc<TokioUdpSocket>,
    message_tx: mpsc::Sender<IncomingMessage>, // 通道现在发送 IncomingMessage
    state: EngineState,
}

impl NetworkHandler {
    pub fn new(
        socket: Arc<TokioUdpSocket>,
        message_tx: mpsc::Sender<IncomingMessage>, // 接收 IncomingMessage Sender
        state: EngineState,
    ) -> Self {
        NetworkHandler {
            socket,
            message_tx,
            state,
        }
    }

    pub async fn receive_messages(&self) {
        // 缓冲区大小固定为 50 字节
        let mut buf = [0u8; MESSAGE_TOTAL_SIZE]; 
        
        loop {
            // 异步等待数据
            match self.socket.recv_from(&mut buf).await {
                Ok((size, _src)) => {
                    // 只处理完整的 50 字节消息
                    if size != MESSAGE_TOTAL_SIZE {
                        eprintln!("[NETWORK] Received incomplete message of size {} bytes. Expected {}.", size, MESSAGE_TOTAL_SIZE);
                        continue;
                    }
                    
                    // 更新总接收计数器
                    let mut total_count_guard = self.state.total_received_orders.lock().await;
                    *total_count_guard += 1;

                    // 使用 Codec 模块解析消息。&buf 已经是 &[u8; 50] 类型，直接传递。
                    if let Err(e) = self.process_single_message(&buf).await {
                        eprintln!("[NETWORK] Error processing message: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("[NETWORK] Error receiving message: {}", e);
                    time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    // 处理单个 50 字节的消息
    // 签名已修复，接受 &buf (即 &[u8; MESSAGE_TOTAL_SIZE])
    async fn process_single_message(&self, buf: &[u8; MESSAGE_TOTAL_SIZE]) -> Result<(), String> {
        // 调用统一的解包函数，现在类型匹配
        match message_codec::unpack_message_payload(buf) {
            Ok(IncomingMessage::Order(order)) => {
                println!("[NETWORK] Parsed Order: {:?}", order);
                // 发送 IncomingMessage::Order
                if let Err(e) = self.message_tx.send(IncomingMessage::Order(order)).await {
                    eprintln!("[NETWORK] Failed to send order to processing queue: {}", e);
                }
            },
            Ok(IncomingMessage::Cancel(cancel_order)) => {
                println!("[NETWORK] Parsed Cancel Order: {:?}", cancel_order);
                // 发送 IncomingMessage::Cancel
                if let Err(e) = self.message_tx.send(IncomingMessage::Cancel(cancel_order)).await {
                    eprintln!("[NETWORK] Failed to send cancel order to processing queue: {}", e);
                }
            },
            Err(e) => {
                eprintln!("[NETWORK] Failed to unpack message: {}", e);
            }
        }
        Ok(())
    }
}
