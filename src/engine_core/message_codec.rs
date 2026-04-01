use alloc::vec::Vec;

use crate::engine_core::types::{
    BroadcastStats, CancelOrder, MatchResult, Order, OrderExecution, MESSAGE_TOTAL_SIZE,
    MSG_ORDER_CANCEL, MSG_ORDER_SUBMIT, MSG_STATUS_BROADCAST, MSG_TRADE_BROADCAST,
};

fn calculate_checksum(buf: &[u8]) -> u8 {
    buf[1..].iter().fold(0, |acc, &byte| acc ^ byte)
}

pub fn serialize_order(order: &Order) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buf = [0u8; MESSAGE_TOTAL_SIZE];
    let payload_start = 2;

    buf[1] = MSG_ORDER_SUBMIT;
    buf[payload_start..payload_start + 2].copy_from_slice(&order.product_id.to_be_bytes());
    buf[payload_start + 2..payload_start + 10].copy_from_slice(&order.order_id.to_be_bytes());
    buf[payload_start + 10..payload_start + 18].copy_from_slice(&order.price.to_be_bytes());
    buf[payload_start + 18..payload_start + 22].copy_from_slice(&order.quantity.to_be_bytes());
    buf[payload_start + 22] = order.order_side;
    buf[payload_start + 23] = order.price_type;
    buf[payload_start + 24..payload_start + 32].copy_from_slice(&order.submit_time.to_be_bytes());
    buf[payload_start + 32..payload_start + 40].copy_from_slice(&order.expire_time.to_be_bytes());
    buf[0] = calculate_checksum(&buf);

    buf
}

pub fn serialize_cancel_order(cancel: &CancelOrder) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buf = [0u8; MESSAGE_TOTAL_SIZE];
    let payload_start = 2;

    buf[1] = MSG_ORDER_CANCEL;
    buf[payload_start..payload_start + 2].copy_from_slice(&cancel.product_id.to_be_bytes());
    buf[payload_start + 2..payload_start + 10].copy_from_slice(&cancel.order_id.to_be_bytes());
    buf[0] = calculate_checksum(&buf);

    buf
}

pub fn serialize_order_execution(result: &OrderExecution) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buf = [0u8; MESSAGE_TOTAL_SIZE];
    let payload_start = 2;

    buf[1] = MSG_TRADE_BROADCAST;
    buf[payload_start..payload_start + 16].copy_from_slice(&result.instance_tag);
    buf[payload_start + 8..payload_start + 10].copy_from_slice(&result.product_id.to_be_bytes());
    buf[payload_start + 10..payload_start + 18].copy_from_slice(&result.buy_order_id.to_be_bytes());
    buf[payload_start + 18..payload_start + 26].copy_from_slice(&result.sell_order_id.to_be_bytes());
    buf[payload_start + 26..payload_start + 34].copy_from_slice(&result.price.to_be_bytes());
    buf[payload_start + 34..payload_start + 38].copy_from_slice(&result.quantity.to_be_bytes());
    buf[payload_start + 38..payload_start + 42]
        .copy_from_slice(&result.trade_time_network.to_be_bytes());
    buf[payload_start + 42..payload_start + 46]
        .copy_from_slice(&result.internal_match_time.to_be_bytes());
    buf[0] = calculate_checksum(&buf);

    buf
}

pub fn serialize_order_execution_share_time(
    result: &OrderExecution,
    time_per_trade: u32,
) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buf = [0u8; MESSAGE_TOTAL_SIZE];
    let payload_start = 2;

    buf[1] = MSG_TRADE_BROADCAST;
    buf[payload_start..payload_start + 16].copy_from_slice(&result.instance_tag);
    buf[payload_start + 16..payload_start + 18].copy_from_slice(&result.product_id.to_be_bytes());
    buf[payload_start + 18..payload_start + 26].copy_from_slice(&result.buy_order_id.to_be_bytes());
    buf[payload_start + 26..payload_start + 34].copy_from_slice(&result.sell_order_id.to_be_bytes());
    buf[payload_start + 34..payload_start + 42].copy_from_slice(&result.price.to_be_bytes());
    buf[payload_start + 42..payload_start + 46].copy_from_slice(&result.quantity.to_be_bytes());
    buf[payload_start + 46..payload_start + 50]
        .copy_from_slice(&result.trade_time_network.to_be_bytes());
    buf[payload_start + 50..payload_start + 54].copy_from_slice(&time_per_trade.to_be_bytes());
    buf[0] = calculate_checksum(&buf);

    buf
}

pub fn serialize_match_result(result: &MatchResult) -> Vec<Vec<u8>> {
    const BATCH_SIZE: usize = 20;

    let mut batches = Vec::new();
    let time_per_trade = result.time_per_order_execution();

    for chunk in result.order_execution_list.chunks(BATCH_SIZE) {
        let mut buf = Vec::with_capacity(MESSAGE_TOTAL_SIZE * chunk.len());
        for trade in chunk {
            let single = serialize_order_execution_share_time(trade, time_per_trade);
            buf.extend_from_slice(&single);
        }
        batches.push(buf);
    }

    batches
}

pub fn serialize_stats_result(stats: &BroadcastStats) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buf = [0u8; MESSAGE_TOTAL_SIZE];
    let payload_start = 2;
    let mut current = payload_start;

    buf[1] = MSG_STATUS_BROADCAST;
    buf[current..current + 16].copy_from_slice(&stats.instance_tag);
    current += 16;
    buf[current..current + 2].copy_from_slice(&stats.product_id.to_be_bytes());
    current += 2;
    buf[current..current + 4].copy_from_slice(&stats.bids_order_count.to_be_bytes());
    current += 4;
    buf[current..current + 4].copy_from_slice(&stats.ask_order_count.to_be_bytes());
    current += 4;
    buf[current..current + 4].copy_from_slice(&stats.matched_orders.to_be_bytes());
    current += 4;
    buf[current..current + 4].copy_from_slice(&stats.total_received_orders.to_be_bytes());
    current += 4;
    buf[current..current + 8].copy_from_slice(&stats.start_time.to_be_bytes());
    current += 8;
    buf[current..current + 4].copy_from_slice(&stats.total_bid_volumn.to_be_bytes());
    current += 4;
    buf[current..current + 4].copy_from_slice(&stats.total_ask_volumn.to_be_bytes());
    buf[0] = calculate_checksum(&buf);

    buf
}

pub fn unpack_message_payload(buf: &[u8; MESSAGE_TOTAL_SIZE]) -> Result<(u8, &[u8]), &'static str> {
    let received_checksum = buf[0];
    let calculated_checksum = calculate_checksum(buf);
    if received_checksum != calculated_checksum {
        return Err("Checksum failed");
    }

    Ok((buf[1], &buf[2..]))
}

pub fn deserialize_order(payload: &[u8]) -> Result<Order, &'static str> {
    if payload.len() < 40 {
        return Err("Order payload too short");
    }

    Ok(Order {
        product_id: u16::from_be_bytes(payload[0..2].try_into().unwrap()),
        order_id: u64::from_be_bytes(payload[2..10].try_into().unwrap()),
        price: u64::from_be_bytes(payload[10..18].try_into().unwrap()),
        quantity: u32::from_be_bytes(payload[18..22].try_into().unwrap()),
        order_side: payload[22],
        price_type: payload[23],
        submit_time: u64::from_be_bytes(payload[24..32].try_into().unwrap()),
        expire_time: u64::from_be_bytes(payload[32..40].try_into().unwrap()),
        _padding: [0u8; 24],
    })
}

pub fn deserialize_cancel_order(payload: &[u8]) -> Result<CancelOrder, &'static str> {
    if payload.len() < 10 {
        return Err("CancelOrder payload too short");
    }

    Ok(CancelOrder {
        product_id: u16::from_be_bytes(payload[0..2].try_into().unwrap()),
        order_id: u64::from_be_bytes(payload[2..10].try_into().unwrap()),
    })
}
