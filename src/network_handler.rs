use crate::data_types::{
    EngineState, IncomingMessage, MESSAGE_TOTAL_SIZE, MSG_ORDER_CANCEL, MSG_ORDER_SUBMIT,
};
use crate::message_codec;

use tokio::net::UdpSocket;
use tokio::sync::mpsc::Sender;

use std::sync::Arc;

/// Handler responsible for receiving incoming network messages (Orders/Cancels).
pub struct NetworkHandler {
    socket: Arc<UdpSocket>,
    sender: Sender<IncomingMessage>,
    state: Arc<EngineState>,
}

impl NetworkHandler {
    /// Creates a new NetworkHandler.
    pub fn new(
        socket: Arc<UdpSocket>,
        sender: Sender<IncomingMessage>,
        state: Arc<EngineState>,
    ) -> Self {
        NetworkHandler {
            socket,
            sender,
            state,
        }
    }

    /// Runs the main loop to receive and process UDP messages.
    pub async fn run_receive_loop(&mut self) {
        let mut buf = [0u8; MESSAGE_TOTAL_SIZE];
        println!("Network handler started, listening for messages...");

        loop {
            match self.socket.recv_from(&mut buf).await {
                Ok((len, sender_addr)) => {
                    if len != MESSAGE_TOTAL_SIZE {
                        eprintln!(
                            "Received packet from {} with incorrect size: {} bytes. Expected {}",
                            sender_addr, len, MESSAGE_TOTAL_SIZE
                        );
                        continue;
                    }

                    println!(
                        "Received packet from {}. Size: {} bytes. Processing...",
                        sender_addr, len
                    );
                    self.process_single_message(&buf).await;
                }
                Err(e) => {
                    eprintln!("UDP receive error: {}", e);
                    // Use a small delay before trying again to prevent a tight loop on continuous errors
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            }
        }
    }

    /// Processes a single 50-byte message packet.
    async fn process_single_message(&self, buf: &[u8; MESSAGE_TOTAL_SIZE]) {
        let msg_result = message_codec::unpack_message_payload(buf);

        if let Err(e) = msg_result {
            eprintln!("Error unpacking message: {}", e);
            return;
        }

        let (message_type, payload) = msg_result.unwrap();

        // Update total received count
        // FIX for E0308: lock().await returns MutexGuard directly, not a Result

        let incoming_message = match message_type {
            MSG_ORDER_SUBMIT => match message_codec::deserialize_order(payload) {
                Ok(order) => {
                    // println!(
                    //     "[LOG] New Order: ProdID={}, Side={}, Price={}, Qty={}",
                    //     order.product_id, order.order_type, order.price, order.quantity
                    // );
                    let mut total_count = self.state.total_received_orders.write().await;
                    println!("deserialize_order");
                    *total_count += 1;
                    IncomingMessage::Order(order)
                }
                Err(e) => {
                    eprintln!("Error deserializing Order: {}", e);
                    return;
                }
            },
            MSG_ORDER_CANCEL => match message_codec::deserialize_cancel_order(payload) {
                Ok(cancel) => {
                    println!(
                        "[LOG] Cancel Order: ProdID={}, OrderID={}",
                        cancel.product_id, cancel.order_id
                    );
                    let mut total_count = self.state.total_received_orders.write().await;
                    *total_count += 1;
                    IncomingMessage::Cancel(cancel)
                }
                Err(e) => {
                    eprintln!("Error deserializing Cancel Order: {}", e);
                    return;
                }
            },
            _ => {
                eprintln!("Unknown message type: {}", message_type);
                return;
            }
        };

        // Send the parsed message to the Order Matcher task
        if let Err(e) = self.sender.send(incoming_message).await {
            eprintln!("Error sending message to matcher: {}", e);
        }
    }
}
