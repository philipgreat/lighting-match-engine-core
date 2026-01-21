use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs}; // <-- 增加 IpAddr
use std::sync::Arc;

use socket2::{Domain, Protocol, Socket, Type};

mod broadcast_handler;
mod data_types;
mod date_time_tool;
mod engine_state;
mod high_resolution_timer;
mod message_codec;
mod network_handler;
mod number_tool;
mod order_book;
mod order_matcher;
mod test_order_book_builder;
use broadcast_handler::TradeNetworkTime;
use data_types::{EngineState, IncomingMessage, MatchResult};

use network_handler::NetworkHandler;
use number_tool::parse_human_readable_u32;
use order_matcher::OrderMatcher;
use test_order_book_builder::TestOrderBookBuilder;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
// use tokio_console::ConsoleLayer;

const DEFAULT_TRADE_ADDR: &str = "239.0.0.1:5000";
const DEFAULT_STATUS_ADDR: &str = "239.0.0.2:5001";
// 监听组播时，绑定地址需要包含端口，但IP通常是0.0.0.0
// 为了简化，我们只监听 trade_addr 或 status_addr 的端口
const DEFAULT_LISTEN_IP: &str = "0.0.0.0";

// --- 新增函数：配置和加入组播组 ---

/// 设置 UDP 套接字并加入指定的组播组。
///
/// `listen_port`: 组播地址的端口 (例如 5000)
/// `multicast_addr`: 组播 IP 地址 (例如 239.0.0.1)
async fn setup_multicast_socket(
    listen_port: u16,
    multicast_addr: Ipv4Addr,
) -> io::Result<UdpSocket> {
    let listen_addr = format!("{}:{}", DEFAULT_LISTEN_IP, listen_port);
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

    // 重要的配置：允许重复使用地址，用于多个进程监听同一端口
    socket.set_reuse_address(true)?;

    // This call now works because SocketExt is in scope:

    let bind_addr: SocketAddr = listen_addr.parse().unwrap();
    socket.bind(&bind_addr.into())?;

    // 加入组播组。
    socket.join_multicast_v4(&multicast_addr, &Ipv4Addr::UNSPECIFIED)?;

    // 转换为 tokio::net::UdpSocket
    let std_socket: std::net::UdpSocket = socket.into();
    std_socket.set_nonblocking(true)?;

    UdpSocket::from_std(std_socket)

    // 绑定到指定的端口和 0.0.0.0 IP
}

// --- 保持 get_config, tag_to_u8_array 等函数不变 ---
// --- 保持 get_config, tag_to_u8_array 等函数不变 ---
fn get_config() -> Result<(String, u16, std::net::SocketAddr, std::net::SocketAddr, u32), String> {
    let args: Vec<String> = std::env::args().collect();
    let mut instance_name = None;
    let mut product_id = None;
    let mut trade_addr_str = None;
    let mut status_addr_str = None;
    let mut test_order_book_size_str = None;

    // Command Line Arguments Parsing
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--name" => {
                if i + 1 < args.len() {
                    instance_name = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--tag" => {
                if i + 1 < args.len() {
                    instance_name = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--prodid" => {
                if i + 1 < args.len() {
                    product_id = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--trade-addr" => {
                if i + 1 < args.len() {
                    trade_addr_str = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--status-addr" => {
                if i + 1 < args.len() {
                    status_addr_str = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--test-order-book-size" => {
                if i + 1 < args.len() {
                    test_order_book_size_str = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    // 1. Instance Name (Tag)
    let tag_string = instance_name
        .or_else(|| std::env::var("INST_NAME").ok())
        .unwrap_or_else(|| "DEFAULT".to_string());

    if tag_string.len() > 16 {
        return Err(format!(
            "Instance tag '{}' exceeds maximum length of 16 characters.",
            tag_string
        ));
    }

    // 2. Product ID
    let prod_id_str = product_id.ok_or_else(|| {
        "Missing required argument: --prodid. Also check env var PROD_ID.".to_string()
    })?;
    let prod_id: u16 = prod_id_str.parse().map_err(|_| {
        format!(
            "Invalid product ID format: '{}'. Must be a valid u16.",
            prod_id_str
        )
    })?;

    // 3. Multicast Addresses
    let trade_addr: std::net::SocketAddr = trade_addr_str
        .unwrap_or_else(|| DEFAULT_TRADE_ADDR.to_string())
        .to_socket_addrs()
        .map_err(|e| format!("Invalid trade address: {}", e))?
        .next()
        .ok_or_else(|| "Could not parse trade address.".to_string())?;

    let status_addr: std::net::SocketAddr = status_addr_str
        .unwrap_or_else(|| DEFAULT_STATUS_ADDR.to_string())
        .to_socket_addrs()
        .map_err(|e| format!("Invalid status address: {}", e))?
        .next()
        .ok_or_else(|| "Could not parse status address.".to_string())?;

    let size_str: &str = test_order_book_size_str
        .as_deref() // Converts Option<String> to Option<&str>
        .unwrap_or("0"); // If None, use "0" as the default &str

    let test_order_book_size: u32 = parse_human_readable_u32(size_str).unwrap_or_else(|e| {
        eprintln!("Error parsing size '{}': {}", size_str, e);
        // Fallback u32 value if the parsing of the string (even the default "0") fails
        0
    });

    Ok((
        tag_string,
        prod_id,
        trade_addr,
        status_addr,
        test_order_book_size,
    ))
}

fn tag_to_u16_array(tag: &str) -> [u8; 16] {
    let mut tag_array = [0u8; 16];
    let bytes = tag.as_bytes();
    let len = std::cmp::min(bytes.len(), 16);
    tag_array[..len].copy_from_slice(&bytes[..len]);
    tag_array
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Lighting Match Engine Core...");

    // 1. Get configuration
    let (tag_string, prod_id, trade_addr, status_addr, test_order_book_size) = match get_config() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Configuration Error: {}", e);
            eprintln!(
                "Usage: --name <tag_8_chars_max> --prodid <u16> [--trade-addr <ip:port>] [--status-addr <ip:port>]"
            );
            return Err(e.into());
        }
    };

    let instance_tag_bytes = tag_to_u16_array(&tag_string);

    println!("Configuration Loaded:");
    println!("  Instance Tag: {}", tag_string);
    println!("  Product ID: {}", prod_id);
    println!("  Trade Multicast: {}", trade_addr);
    println!("  Status Multicast: {}", status_addr);
    println!("--------------------------------------------------");

    // 2. Initialize Sockets and JOIN Multicast Group

    // --- 修改点 A：为输入套接字（接收 Trade）加入组播组 ---
    let trade_ip = match trade_addr.ip() {
        IpAddr::V4(ip) => ip,
        _ => return Err("Trade multicast must be IPv4".into()),
    };
    let input_socket = setup_multicast_socket(trade_addr.port(), trade_ip).await?;
    let shared_input_socket = Arc::new(input_socket);
    println!(
        "✅ Input socket bound to {} and joined trade group: {}",
        shared_input_socket.local_addr()?,
        trade_addr
    );

    let broadcast_socket = UdpSocket::bind("0.0.0.0:0").await?; // 绑定到任意端口
    broadcast_socket.set_multicast_ttl_v4(64)?;
    let shared_broadcast_socket = Arc::new(broadcast_socket);
    println!(
        "✅ Broadcast socket bound to {}",
        shared_broadcast_socket.local_addr()?
    );
    println!("--------------------------------------------------");

    // 3. Initialize Engine State
    let engine_state = Arc::new(EngineState::new(instance_tag_bytes, prod_id, status_addr));

    let mut test_order_book_builder =
        TestOrderBookBuilder::new(test_order_book_size, engine_state.clone());

    test_order_book_builder.start_run().await;

    // 4. Initialize Channels
    let (message_tx, message_rx) = mpsc::channel::<IncomingMessage>(1024);
    let (match_tx, match_rx) = mpsc::channel::<MatchResult>(1024);

    // 5. Initialize Handlers
    let mut network_handler = NetworkHandler::new(
        shared_input_socket.clone(),
        message_tx,
        engine_state.clone(),
    );
    let mut order_matcher = OrderMatcher::new(message_rx, match_tx, engine_state.clone());
    let mut broadcast_handler = TradeNetworkTime::new(
        shared_broadcast_socket.clone(),
        engine_state.status_multicast_addr,
        match_rx,
    );
    let status_broadcaster =
        EngineState::new_status_broadcaster(engine_state.clone(), shared_broadcast_socket.clone());

    // 6. Run all tasks concurrently
    tokio::select! {
        _ = network_handler.run_receive_loop() => { println!("Network receiver exited."); }
        _ = order_matcher.run_matching_loop() => { println!("Order matcher exited."); }
        _ = broadcast_handler.run_broadcast_loop() => { println!("Broadcast handler exited."); }
        _ = status_broadcaster.run_status_broadcast() => { println!("Status broadcaster exited."); }
    }
    print!("runs here");

    Ok(())
}
