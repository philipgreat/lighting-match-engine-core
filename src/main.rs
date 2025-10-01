// 声明 modules
mod data_types;
mod engine_state;
mod network_handler;
mod order_matcher;
mod broadcast_handler; 
mod message_codec; 

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket as TokioUdpSocket;
use tokio::sync::mpsc;
use tokio::task;

use data_types::{MatchResult}; // 引入 IncomingMessage
use crate::data_types::*;
use network_handler::NetworkHandler;
use order_matcher::OrderMatcher;
use broadcast_handler::BroadcastHandler; 

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // 1. 初始化核心状态和通道
    let multicast_addr = "224.0.0.1:5000";
    
    // 消息接收 -> 撮合处理 通道 (现在发送 IncomingMessage)
    let (message_tx, message_rx) = mpsc::channel(1000); 
    
    // 撮合处理 -> 广播发送 通道
    let (match_tx, match_rx) = mpsc::channel::<MatchResult>(1000); 

    println!("Starting matching engine on {}...", multicast_addr);

    // 2. 初始化网络 Socket
    let (ip_str, port_str) = multicast_addr
        .split_once(':')
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid multicast group format")
        })?;
    let port: u16 = port_str.parse().map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("Invalid port: {}", e))
    })?;
    let multicast_ip: Ipv4Addr = ip_str.parse().map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("Invalid IP: {}", e))
    })?;

    // 绑定并加入多播组 (用于接收和发送)
    let socket = TokioUdpSocket::bind(format!("0.0.0.0:{}", port)).await?;
    socket.join_multicast_v4(multicast_ip, Ipv4Addr::new(0, 0, 0, 0))?;
    let socket_arc = Arc::new(socket); 
    
    // 实例化 EngineState，传入 Socket 和地址
    let engine_state = EngineState::new(1, socket_arc.clone(), multicast_addr.to_string()); 

    // 3. 创建各个处理器实例
    let network_handler = NetworkHandler::new(socket_arc.clone(), message_tx, engine_state.clone());
    let order_matcher = OrderMatcher::new(engine_state.clone(), match_tx); 
    let broadcast_handler = BroadcastHandler::new(socket_arc.clone(), multicast_addr.to_string()); 

    // 4. 启动任务
    
    // 任务 1: 消息接收 (Message Receive)
    let receive_task = task::spawn(async move {
        network_handler.receive_messages().await; 
    });

    // 任务 2: 撮合处理 (Order Matching)
    let process_task = task::spawn(async move {
        order_matcher.process_orders(message_rx).await;
    });

    // 任务 3: 统计广播 (Status Broadcast)
    let stats_task = task::spawn(async move {
        loop {
            engine_state.broadcast_stats().await;
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });

    // 任务 4: 成交广播 (Trade Broadcast)
    let broadcast_task = task::spawn(async move {
        broadcast_handler.start_broadcasting(match_rx).await;
    });

    // 等待任务完成
    tokio::select! {
        _ = receive_task => println!("Receive task finished."),
        _ = process_task => println!("Process task finished."),
        _ = stats_task => println!("Stats task finished."),
        _ = broadcast_task => println!("Broadcast task finished."), 
    }
    
    Ok(())
}
