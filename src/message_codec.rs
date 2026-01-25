use crate::data_types::{
    BroadcastStats, CancelOrder, MESSAGE_TOTAL_SIZE, MSG_ORDER_CANCEL, MSG_ORDER_SUBMIT,
    MSG_STATUS_BROADCAST, MSG_TRADE_BROADCAST, Trade, Order,MatchResult
};

/// Calculates a simple XOR checksum for the payload starting after the type byte (index 2).
/// The buffer must be at least 2 bytes long.
fn calculate_checksum(buf: &[u8]) -> u8 {
    // Checksum is calculated over the payload (index 2 onwards)
    buf[1..].iter().fold(0, |acc, &x| acc ^ x)
}

/// Serializes an Order struct into a 50-byte network buffer.
pub fn serialize_order(order: &Order) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buf = [0u8; MESSAGE_TOTAL_SIZE];
    let payload_start = 2; // Checksum (0) + Type (1) = Start at index 2

    buf[1] = MSG_ORDER_SUBMIT;

    // Product ID (u16)
    buf[payload_start..payload_start + 2].copy_from_slice(&order.product_id.to_be_bytes());
    // Order ID (u64)
    buf[payload_start + 2..payload_start + 10].copy_from_slice(&order.order_id.to_be_bytes());
    // Price (u64)
    buf[payload_start + 10..payload_start + 18].copy_from_slice(&order.price.to_be_bytes());
    // Quantity (u32)
    buf[payload_start + 18..payload_start + 22].copy_from_slice(&order.quantity.to_be_bytes());
    // Order Type (u8)
    buf[payload_start + 22] = order.order_type;
    // Price Type (u8)
    buf[payload_start + 23] = order.price_type;
    // Submit Time (u64)
    buf[payload_start + 24..payload_start + 32].copy_from_slice(&order.submit_time.to_be_bytes());
    // Expire Time (u64)
    buf[payload_start + 32..payload_start + 40].copy_from_slice(&order.expire_time.to_be_bytes());

    // Checksum calculation and placement
    buf[0] = calculate_checksum(&buf);

    buf
}

/// Serializes a CancelOrder struct into a 50-byte network buffer.
pub fn serialize_cancel_order(cancel: &CancelOrder) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buf = [0u8; MESSAGE_TOTAL_SIZE];
    let payload_start = 2;

    buf[1] = MSG_ORDER_CANCEL;

    // Product ID (u16)
    buf[payload_start..payload_start + 2].copy_from_slice(&cancel.product_id.to_be_bytes());
    // Order ID (u64)
    buf[payload_start + 2..payload_start + 10].copy_from_slice(&cancel.order_id.to_be_bytes());

    // Checksum calculation and placement
    buf[0] = calculate_checksum(&buf);

    buf
}

/// Serializes a Trade struct into a 50-byte network buffer.
pub fn serialize_single_trade(result: &Trade) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buf = [0u8; MESSAGE_TOTAL_SIZE];
    let payload_start = 2;

    buf[1] = MSG_TRADE_BROADCAST;

    // Instance Tag ([u8; 8])
    buf[payload_start..payload_start + 16].copy_from_slice(&result.instance_tag);
    // Product ID (u16)
    buf[payload_start + 8..payload_start + 10].copy_from_slice(&result.product_id.to_be_bytes());
    // Buy Order ID (u64)
    buf[payload_start + 10..payload_start + 18].copy_from_slice(&result.buy_order_id.to_be_bytes());
    // Sell Order ID (u64)
    buf[payload_start + 18..payload_start + 26]
        .copy_from_slice(&result.sell_order_id.to_be_bytes());
    // Price (u64)
    buf[payload_start + 26..payload_start + 34].copy_from_slice(&result.price.to_be_bytes());
    // Quantity (u32)
    buf[payload_start + 34..payload_start + 38].copy_from_slice(&result.quantity.to_be_bytes());
    // Trade Time (u64)
    buf[payload_start + 38..payload_start + 42]
        .copy_from_slice(&result.trade_time_network.to_be_bytes());
    buf[payload_start + 42..payload_start + 46]
        .copy_from_slice(&result.internal_match_time.to_be_bytes());
    // Padding to 50 bytes is implicit by the array size (index 48 is the last element used)

    // Checksum calculation and placement
    buf[0] = calculate_checksum(&buf);

    buf
}

pub fn serialize_single_trade_share_time(result: &Trade, time_per_trade: u32) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buf = [0u8; MESSAGE_TOTAL_SIZE];
    let payload_start = 2;

    buf[1] = MSG_TRADE_BROADCAST;

    // Instance Tag ([u8; 8])
    buf[payload_start..payload_start + 16].copy_from_slice(&result.instance_tag);
    // Product ID (u16)
    buf[payload_start + 8..payload_start + 10].copy_from_slice(&result.product_id.to_be_bytes());
    // Buy Order ID (u64)
    buf[payload_start + 10..payload_start + 18].copy_from_slice(&result.buy_order_id.to_be_bytes());
    // Sell Order ID (u64)
    buf[payload_start + 18..payload_start + 26]
        .copy_from_slice(&result.sell_order_id.to_be_bytes());
    // Price (u64)
    buf[payload_start + 26..payload_start + 34].copy_from_slice(&result.price.to_be_bytes());
    // Quantity (u32)
    buf[payload_start + 34..payload_start + 38].copy_from_slice(&result.quantity.to_be_bytes());
    // Trade Time (u64)
    buf[payload_start + 38..payload_start + 42]
        .copy_from_slice(&result.trade_time_network.to_be_bytes());
    buf[payload_start + 42..payload_start + 46]
        .copy_from_slice(&time_per_trade.to_be_bytes());
    // Padding to 50 bytes is implicit by the array size (index 48 is the last element used)

    // Checksum calculation and placement
    buf[0] = calculate_checksum(&buf);

    buf
}

pub fn serialize_match_result(result: &MatchResult,) -> Vec<Vec<u8>> {
    const BATCH_SIZE: usize = 20;

    let mut batches = Vec::new();
    
    let time_per_trade = result.time_per_trade();
    for chunk in result.trade_list.chunks(BATCH_SIZE) {
        let mut buf =
            Vec::with_capacity(MESSAGE_TOTAL_SIZE * chunk.len());
        
        for trade in chunk {
            let single = serialize_single_trade_share_time(trade,time_per_trade);
            buf.extend_from_slice(&single);
        }

        batches.push(buf);
    }

    batches
}


/// Serializes a BroadcastStats struct into a 50-byte network buffer.
pub fn serialize_stats_result(stats: &BroadcastStats) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buf = [0u8; MESSAGE_TOTAL_SIZE];

    // Payload starts after Checksum (1 byte) and Message Type (1 byte)
    let payload_start_idx = 2;
    let mut current_idx = payload_start_idx;

    // Assuming MSG_STATUS_BROADCAST and calculate_checksum are defined elsewhere
    buf[1] = MSG_STATUS_BROADCAST;

    // --- Payload Serialization (Total 30 bytes) ---

    // 1. Instance Tag ([u8; 8])
    // Size: 16 bytes
    buf[current_idx..current_idx + 16].copy_from_slice(&stats.instance_tag);
    current_idx += 16; // Index: 18

    // 2. Product ID (u16)
    // Size: 2 bytes
    buf[current_idx..current_idx + 2].copy_from_slice(&stats.product_id.to_be_bytes());
    current_idx += 2; // Index: 20

    // 3. Order Book Size (u32)
    // Size: 4 bytes (FIXED from u64)
    buf[current_idx..current_idx + 4].copy_from_slice(&stats.bids_size.to_be_bytes());
    current_idx += 4; // Index: 24

    buf[current_idx..current_idx + 4].copy_from_slice(&stats.ask_size.to_be_bytes());
    current_idx += 4; // Index: 28

    // 4. Matched Orders (u32)
    // Size: 4 bytes (FIXED from u64)
    buf[current_idx..current_idx + 4].copy_from_slice(&stats.matched_orders.to_be_bytes());
    current_idx += 4; // Index: 32

    // 5. Total Received Orders (u32)
    // Size: 4 bytes (FIXED from u64)
    buf[current_idx..current_idx + 4].copy_from_slice(&stats.total_received_orders.to_be_bytes());
    current_idx += 4; // Index: 36

    // 6. Start Time (u64)
    // Size: 8 bytes
    buf[current_idx..current_idx + 8].copy_from_slice(&stats.start_time.to_be_bytes());
    //current_idx += 8; // Index: 32 (Last index written: 31)

    // Checksum calculation and placement
    // Last data byte is at index 31. Padding goes from index 32 up to MESSAGE_TOTAL_SIZE - 1.
    buf[0] = calculate_checksum(&buf);

    buf
}

/// Unpacks a 50-byte network buffer into an Order or CancelOrder payload.
/// Performs checksum validation and returns the message type and payload slice.
pub fn unpack_message_payload(buf: &[u8; MESSAGE_TOTAL_SIZE]) -> Result<(u8, &[u8]), &'static str> {
    if buf.len() != MESSAGE_TOTAL_SIZE {
        return Err("Buffer size mismatch");
    }

    let received_checksum = buf[0];
    let calculated_checksum = calculate_checksum(buf);

    if received_checksum != calculated_checksum {
        return Err("Checksum failed");
    }

    let message_type = buf[1];
    let payload = &buf[2..];

    Ok((message_type, payload))
}

/// Deserializes a payload slice into an Order struct.
pub fn deserialize_order(payload: &[u8]) -> Result<Order, &'static str> {
    if payload.len() < 40 {
        return Err("Order payload too short");
    }

    let product_id = u16::from_be_bytes(payload[0..2].try_into().unwrap());
    let order_id = u64::from_be_bytes(payload[2..10].try_into().unwrap());
    let price = u64::from_be_bytes(payload[10..18].try_into().unwrap());
    let quantity = u32::from_be_bytes(payload[18..22].try_into().unwrap());
    let order_type = payload[22];
    let price_type = payload[23];
    let submit_time = u64::from_be_bytes(payload[24..32].try_into().unwrap());
    let expire_time = u64::from_be_bytes(payload[32..40].try_into().unwrap());
    let is_mocked_order = payload[22] > 2;
    Ok(Order {
        product_id,
        order_id,
        price,
        quantity,
        order_type,
        price_type,
        submit_time,
        expire_time,
        is_mocked_order,
    })
}

/// Deserializes a payload slice into a CancelOrder struct.
pub fn deserialize_cancel_order(payload: &[u8]) -> Result<CancelOrder, &'static str> {
    if payload.len() < 10 {
        return Err("CancelOrder payload too short");
    }

    let product_id = u16::from_be_bytes(payload[0..2].try_into().unwrap());
    let order_id = u64::from_be_bytes(payload[2..10].try_into().unwrap());

    Ok(CancelOrder {
        product_id,
        order_id,
    })
}
