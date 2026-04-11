use crate::types::{
    BroadcastStats, CancelOrder, MatchOutcome, OrderBookError, OrderFlags, OrderRequest,
    OrderSide, OrderSubmitError, PriceType, Trade, MESSAGE_TOTAL_SIZE, MSG_ERROR_REPLY,
    MSG_ORDER_CANCEL, MSG_ORDER_SUBMIT, MSG_STATUS_BROADCAST, MSG_TRADE_BROADCAST,
};

#[derive(Debug, Clone)]
pub struct TradeBroadcast {
    pub instance_tag: [u8; 16],
    pub product_id: u16,
    pub buy_order_id: u64,
    pub sell_order_id: u64,
    pub price: u64,
    pub quantity: u32,
    pub trade_time_network: u32,
    pub internal_match_time: u32,
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderBookErrorCode {
    PriceOutOfRange = 1001,
    PriceNotOnTick = 1002,
}

#[derive(Debug, Clone)]
pub struct OrderBookErrorReply {
    pub instance_tag: [u8; 16],
    pub product_id: u16,
    pub order_id: u64,
    pub error_code: OrderBookErrorCode,
    pub message: [u8; 32],
}

impl OrderBookErrorReply {
    pub fn from_submit_error(instance_tag: [u8; 16], error: &OrderSubmitError) -> Self {
        let (error_code, label) = map_order_book_error(&error.source);
        let mut message = [0u8; 32];
        let bytes = label.as_bytes();
        let len = bytes.len().min(message.len());
        message[..len].copy_from_slice(&bytes[..len]);

        Self {
            instance_tag,
            product_id: error.order.product_id,
            order_id: error.order.order_id,
            error_code,
            message,
        }
    }
}

pub fn map_order_book_error(error: &OrderBookError) -> (OrderBookErrorCode, &'static str) {
    match error {
        OrderBookError::PriceOutOfRange { .. } => {
            (OrderBookErrorCode::PriceOutOfRange, "PRICE_OUT_OF_RANGE")
        }
        OrderBookError::PriceNotOnTick { .. } => {
            (OrderBookErrorCode::PriceNotOnTick, "PRICE_NOT_ON_TICK")
        }
    }
}

pub fn format_order_submit_error_cli(error: &OrderSubmitError) -> String {
    let (code, label) = map_order_book_error(&error.source);
    format!(
        "ORDER_REJECT code={} label={} product_id={} order_id={} detail={}",
        code as u16,
        label,
        error.order.product_id,
        error.order.order_id,
        error.source
    )
}

impl TradeBroadcast {
    pub fn from_trade(
        instance_tag: [u8; 16],
        trade: &Trade,
        trade_time_network: u32,
        internal_match_time: u32,
    ) -> Self {
        Self {
            instance_tag,
            product_id: trade.product_id,
            buy_order_id: trade.buy_order_id,
            sell_order_id: trade.sell_order_id,
            price: trade.price,
            quantity: trade.quantity,
            trade_time_network,
            internal_match_time,
        }
    }
}

fn calculate_checksum(buf: &[u8]) -> u8 {
    buf[1..].iter().fold(0, |acc, &x| acc ^ x)
}

pub fn serialize_order(order: &OrderRequest) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buf = [0u8; MESSAGE_TOTAL_SIZE];
    let payload_start = 2;

    buf[1] = MSG_ORDER_SUBMIT;
    buf[payload_start..payload_start + 2].copy_from_slice(&order.product_id.to_be_bytes());
    buf[payload_start + 2..payload_start + 10].copy_from_slice(&order.order_id.to_be_bytes());
    buf[payload_start + 10..payload_start + 18].copy_from_slice(&order.price.to_be_bytes());
    buf[payload_start + 18..payload_start + 22].copy_from_slice(&order.quantity.to_be_bytes());
    buf[payload_start + 22] = order.side.to_wire(order.flags.is_mock);
    buf[payload_start + 23] = order.price_type.to_wire();
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

pub fn serialize_trade_broadcast(broadcast: &TradeBroadcast) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buf = [0u8; MESSAGE_TOTAL_SIZE];
    let payload_start = 2;

    buf[1] = MSG_TRADE_BROADCAST;
    buf[payload_start..payload_start + 16].copy_from_slice(&broadcast.instance_tag);
    buf[payload_start + 16..payload_start + 18]
        .copy_from_slice(&broadcast.product_id.to_be_bytes());
    buf[payload_start + 18..payload_start + 26]
        .copy_from_slice(&broadcast.buy_order_id.to_be_bytes());
    buf[payload_start + 26..payload_start + 34]
        .copy_from_slice(&broadcast.sell_order_id.to_be_bytes());
    buf[payload_start + 34..payload_start + 42].copy_from_slice(&broadcast.price.to_be_bytes());
    buf[payload_start + 42..payload_start + 46]
        .copy_from_slice(&broadcast.quantity.to_be_bytes());
    buf[payload_start + 46..payload_start + 50]
        .copy_from_slice(&broadcast.trade_time_network.to_be_bytes());
    buf[payload_start + 50..payload_start + 54]
        .copy_from_slice(&broadcast.internal_match_time.to_be_bytes());
    buf[0] = calculate_checksum(&buf);

    buf
}

pub fn serialize_order_book_error_reply(reply: &OrderBookErrorReply) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buf = [0u8; MESSAGE_TOTAL_SIZE];
    let payload_start = 2;

    buf[1] = MSG_ERROR_REPLY;
    buf[payload_start..payload_start + 16].copy_from_slice(&reply.instance_tag);
    buf[payload_start + 16..payload_start + 18].copy_from_slice(&reply.product_id.to_be_bytes());
    buf[payload_start + 18..payload_start + 26].copy_from_slice(&reply.order_id.to_be_bytes());
    buf[payload_start + 26..payload_start + 28]
        .copy_from_slice(&(reply.error_code as u16).to_be_bytes());
    buf[payload_start + 28..payload_start + 60].copy_from_slice(&reply.message);
    buf[0] = calculate_checksum(&buf);

    buf
}

pub fn serialize_match_outcome(
    instance_tag: [u8; 16],
    outcome: &MatchOutcome,
) -> Vec<Vec<u8>> {
    const BATCH_SIZE: usize = 20;

    let mut batches = Vec::new();
    let shared_internal_match_time = outcome.time_per_trade();

    for chunk in outcome.trades.chunks(BATCH_SIZE) {
        let mut buf = Vec::with_capacity(MESSAGE_TOTAL_SIZE * chunk.len());
        for trade in chunk {
            let broadcast = TradeBroadcast::from_trade(
                instance_tag,
                trade,
                0,
                shared_internal_match_time,
            );
            let single = serialize_trade_broadcast(&broadcast);
            buf.extend_from_slice(&single);
        }
        batches.push(buf);
    }

    batches
}

pub fn serialize_stats_result(stats: &BroadcastStats) -> [u8; MESSAGE_TOTAL_SIZE] {
    let mut buf = [0u8; MESSAGE_TOTAL_SIZE];
    let payload_start_idx = 2;
    let mut current_idx = payload_start_idx;

    buf[1] = MSG_STATUS_BROADCAST;
    buf[current_idx..current_idx + 16].copy_from_slice(&stats.instance_tag);
    current_idx += 16;
    buf[current_idx..current_idx + 2].copy_from_slice(&stats.product_id.to_be_bytes());
    current_idx += 2;
    buf[current_idx..current_idx + 4].copy_from_slice(&stats.bids_order_count.to_be_bytes());
    current_idx += 4;
    buf[current_idx..current_idx + 4].copy_from_slice(&stats.ask_order_count.to_be_bytes());
    current_idx += 4;
    buf[current_idx..current_idx + 4].copy_from_slice(&stats.matched_orders.to_be_bytes());
    current_idx += 4;
    buf[current_idx..current_idx + 4].copy_from_slice(&stats.total_received_orders.to_be_bytes());
    current_idx += 4;
    buf[current_idx..current_idx + 8].copy_from_slice(&stats.start_time.to_be_bytes());
    current_idx += 8;
    buf[current_idx..current_idx + 4].copy_from_slice(&stats.total_bid_volumn.to_be_bytes());
    current_idx += 4;
    buf[current_idx..current_idx + 4].copy_from_slice(&stats.total_ask_volumn.to_be_bytes());
    buf[0] = calculate_checksum(&buf);

    buf
}

pub fn unpack_message_payload(
    buf: &[u8; MESSAGE_TOTAL_SIZE],
) -> Result<(u8, &[u8]), &'static str> {
    if buf.len() != MESSAGE_TOTAL_SIZE {
        return Err("Buffer size mismatch");
    }

    let received_checksum = buf[0];
    let calculated_checksum = calculate_checksum(buf);

    if received_checksum != calculated_checksum {
        return Err("Checksum failed");
    }

    Ok((buf[1], &buf[2..]))
}

pub fn deserialize_order(payload: &[u8]) -> Result<OrderRequest, &'static str> {
    if payload.len() < 40 {
        return Err("Order payload too short");
    }

    let (side, is_mock) = OrderSide::from_wire(payload[22])?;
    let price_type = PriceType::from_wire(payload[23])?;

    Ok(OrderRequest {
        product_id: u16::from_be_bytes(payload[0..2].try_into().unwrap()),
        order_id: u64::from_be_bytes(payload[2..10].try_into().unwrap()),
        price: u64::from_be_bytes(payload[10..18].try_into().unwrap()),
        quantity: u32::from_be_bytes(payload[18..22].try_into().unwrap()),
        side,
        price_type,
        flags: OrderFlags { is_mock },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{OrderBookError, OrderFlags, OrderRequest, OrderSide, OrderSubmitError, PriceType};

    fn sample_order() -> OrderRequest {
        OrderRequest {
            product_id: 7,
            side: OrderSide::Buy,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            quantity: 1,
            order_id: 42,
            price: 105,
            submit_time: 0,
            expire_time: 0,
            _padding: [0u8; 24],
        }
    }

    #[test]
    fn maps_order_book_errors_to_stable_codes_and_labels() {
        let range = map_order_book_error(&OrderBookError::PriceOutOfRange {
            price: 50,
            min_price: 100,
            max_price: 130,
        });
        assert_eq!(range, (OrderBookErrorCode::PriceOutOfRange, "PRICE_OUT_OF_RANGE"));

        let tick = map_order_book_error(&OrderBookError::PriceNotOnTick {
            price: 105,
            base_price: 100,
            tick: 10,
        });
        assert_eq!(tick, (OrderBookErrorCode::PriceNotOnTick, "PRICE_NOT_ON_TICK"));
    }

    #[test]
    fn formats_cli_rejection_with_code_label_and_order_identity() {
        let err = OrderSubmitError {
            order: sample_order(),
            source: OrderBookError::PriceNotOnTick {
                price: 105,
                base_price: 100,
                tick: 10,
            },
        };

        let formatted = format_order_submit_error_cli(&err);
        assert!(formatted.contains("ORDER_REJECT"));
        assert!(formatted.contains("code=1002"));
        assert!(formatted.contains("label=PRICE_NOT_ON_TICK"));
        assert!(formatted.contains("product_id=7"));
        assert!(formatted.contains("order_id=42"));
    }

    #[test]
    fn serializes_error_reply_with_message_code_and_error_code() {
        let err = OrderSubmitError {
            order: sample_order(),
            source: OrderBookError::PriceNotOnTick {
                price: 105,
                base_price: 100,
                tick: 10,
            },
        };
        let reply = OrderBookErrorReply::from_submit_error(*b"engine-instance!", &err);
        let buf = serialize_order_book_error_reply(&reply);

        assert_eq!(buf[1], MSG_ERROR_REPLY);
        assert_eq!(u16::from_be_bytes([buf[28], buf[29]]), 1002);
        assert_eq!(u16::from_be_bytes([buf[18], buf[19]]), 7);
        assert_eq!(u64::from_be_bytes(buf[20..28].try_into().unwrap()), 42);
    }
}
