use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs}; // <-- 增加 IpAddr
use std::sync::Arc;

mod data_types;
mod date_time_tool;
mod engine_state;
mod high_resolution_timer;
mod message_codec;
mod number_tool;
mod continuous_order_book;
mod call_auction_pool;
mod test_order_book_builder;

use data_types::{EngineState, IncomingMessage, MatchResult};

use number_tool::parse_human_readable_u32;

use test_order_book_builder::TestOrderBookBuilder;
// use tokio_console::ConsoleLayer;
/// `listen_port`: 组播地址的端口 (例如 5000)
/// `multicast_addr`: 组播 IP 地址 (例如 239.0.0.1)


// --- 保持 get_config, tag_to_u8_array 等函数不变 ---
// --- 保持 get_config, tag_to_u8_array 等函数不变 ---
fn get_config() -> Result<(String, u16, u32), String> {
    let args: Vec<String> = std::env::args().collect();
    let mut instance_name = None;
    let mut product_id = None;
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

 fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Lighting Match Engine Core...");

    // 1. Get configuration
    let (tag_string, prod_id, test_order_book_size) = match get_config() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Configuration Error: {}", e);
            eprintln!(
                "Usage: --name <tag_16_chars_max> --prodid <u16> [--trade-addr <ip:port>] [--status-addr <ip:port>]"
            );
            return Err(e.into());
        }
    };

    let instance_tag_bytes = tag_to_u16_array(&tag_string);

    println!("Configuration Loaded:");
    println!("  Instance Tag: {}", tag_string);
    println!("  Product ID: {}", prod_id);
    println!("--------------------------------------------------");

    // 2. Initialize Sockets and JOIN Multicast Group

    println!("--------------------------------------------------");

    // 3. Initialize Engine State
    let  engine_state = EngineState::new(instance_tag_bytes, prod_id);

    let test_order_book_builder = TestOrderBookBuilder::new(test_order_book_size);

    test_order_book_builder.start_run(engine_state);
    


    print!("runs here");

    Ok(())
}
