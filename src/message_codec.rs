use crate::data_types::*;

// 统一消息总大小 (50 字节)
pub const MESSAGE_TOTAL_SIZE: usize = 50;

// 负载大小常量 (用于校验)
pub const ORDER_PAYLOAD_SIZE: usize = 40;     // New size: 40 bytes
pub const CANCEL_PAYLOAD_SIZE: usize = 10;
pub const MATCH_PAYLOAD_SIZE: usize = 30;
pub const STATS_PAYLOAD_SIZE: usize = 34;     // New size: 34 bytes

// --- 序列化 (打包) 逻辑 ---

// 序列化 MatchResult (30 字节负载) 成 50 字节二进制包
pub fn serialize_match_result(result: &MatchResult) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buffer = [0u8; MESSAGE_TOTAL_SIZE];
    
    // 1. 消息头 (1 字节)
    buffer[0] = MSG_TRADE_BROADCAST;
    
    let mut offset = 1;
    
    // 2. 消息体 (30 字节)
    buffer[offset..offset + 2].copy_from_slice(&result.product_id.to_be_bytes());
    offset += 2;
    buffer[offset..offset + 8].copy_from_slice(&result.buy_order_id.to_be_bytes());
    offset += 8;
    buffer[offset..offset + 8].copy_from_slice(&result.sell_order_id.to_be_bytes());
    offset += 8;
    buffer[offset..offset + 8].copy_from_slice(&result.price.to_be_bytes());
    offset += 8;
    buffer[offset..offset + 4].copy_from_slice(&result.quantity.to_be_bytes());
    // offset += 4; // Total payload: 30 bytes + 1 byte header = 31 bytes

    buffer
}

// 序列化 BroadcastStats (34 字节负载) 成 50 字节二进制包
pub fn serialize_stats_result(stats: &BroadcastStats) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buffer = [0u8; MESSAGE_TOTAL_SIZE];
    
    // 1. 消息头 (1 字节)
    buffer[0] = MSG_STATUS_BROADCAST;
    
    let mut offset = 1;

    // 2. 消息体 (34 字节)
    buffer[offset..offset + 2].copy_from_slice(&stats.product_id.to_be_bytes());
    offset += 2;
    buffer[offset..offset + 8].copy_from_slice(&stats.order_book_size.to_be_bytes());
    offset += 8;
    buffer[offset..offset + 8].copy_from_slice(&stats.matched_orders.to_be_bytes());
    offset += 8;
    buffer[offset..offset + 8].copy_from_slice(&stats.total_received_orders.to_be_bytes());
    offset += 8;
    buffer[offset..offset + 8].copy_from_slice(&stats.start_time.to_be_bytes());
    // offset += 8; // Total payload: 34 bytes + 1 byte header = 35 bytes

    buffer
}


// --- 反序列化 (解包) 逻辑 ---

// 解包 50 字节二进制包并返回相应的 IncomingMessage 变体
pub fn unpack_message_payload(buffer: &[u8; MESSAGE_TOTAL_SIZE]) -> Result<IncomingMessage, String> {
    let message_type = buffer[0];
    let payload = &buffer[1..];
    
    match message_type {
        MSG_ORDER_SUBMIT => {
            if payload.len() < ORDER_PAYLOAD_SIZE {
                return Err(format!("Order payload too small: {} bytes", payload.len()));
            }
            Ok(IncomingMessage::Order(deserialize_order(payload)))
        },
        MSG_ORDER_CANCEL => {
            if payload.len() < CANCEL_PAYLOAD_SIZE {
                 return Err(format!("Cancel payload too small: {} bytes", payload.len()));
            }
            Ok(IncomingMessage::Cancel(deserialize_cancel_order(payload)))
        },
        _ => Err(format!("Unknown message type: {}", message_type)),
    }
}

fn deserialize_order(payload: &[u8]) -> Order {
    let mut offset = 0;

    let product_id = u16::from_be_bytes(payload[offset..offset + 2].try_into().unwrap());
    offset += 2;
    let order_id = u64::from_be_bytes(payload[offset..offset + 8].try_into().unwrap());
    offset += 8;
    let price = u64::from_be_bytes(payload[offset..offset + 8].try_into().unwrap());
    offset += 8;
    let quantity = u32::from_be_bytes(payload[offset..offset + 4].try_into().unwrap());
    offset += 4;
    let order_type = payload[offset]; // New: Order Direction (BUY/SELL)
    offset += 1;
    let price_type = payload[offset]; // New: Price Type (LIMIT/MARKET)
    offset += 1;
    let submit_time = u64::from_be_bytes(payload[offset..offset + 8].try_into().unwrap());
    offset += 8;
    let expire_time = u64::from_be_bytes(payload[offset..offset + 8].try_into().unwrap());
    // offset += 8; // Total 40 bytes

    Order {
        product_id,
        order_id,
        price,
        quantity,
        order_type,
        price_type,
        submit_time,
        expire_time,
    }
}

fn deserialize_cancel_order(payload: &[u8]) -> CancelOrder {
    let mut offset = 0;

    let product_id = u16::from_be_bytes(payload[offset..offset + 2].try_into().unwrap());
    offset += 2;
    let order_id = u64::from_be_bytes(payload[offset..offset + 8].try_into().unwrap());
    // offset += 8;

    CancelOrder {
        product_id,
        order_id,
    }
}
