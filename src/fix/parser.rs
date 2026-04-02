use std::collections::HashMap;

use super::model::{
    price_type_from_fix, side_from_fix, CancelOrderRequest, FixIncomingMessage, NewOrderRequest,
    ReplaceOrderRequest, StatusRequest,
};

pub fn parse_fix_message(input: &str) -> Result<FixIncomingMessage, String> {
    let fields = parse_fix_fields(input);
    let msg_type = required_field(&fields, "35")?;

    match msg_type {
        "D" => parse_new_order(&fields),
        "F" => parse_cancel_order(&fields),
        "G" => parse_replace_order(&fields),
        "H" => parse_status_request(&fields),
        other => Err(format!("unsupported MsgType(35): {other}")),
    }
}

fn parse_new_order(fields: &HashMap<String, String>) -> Result<FixIncomingMessage, String> {
    let order_id = parse_required_u64(fields, "11", "ClOrdID")?;
    let side = side_from_fix(required_field(fields, "54")?)?;
    let quantity = parse_required_u32(fields, "38", "OrderQty")?;
    let price_type = price_type_from_fix(required_field(fields, "40")?)?;
    let price = match price_type {
        crate::data_types::ORDER_PRICE_TYPE_LIMIT => parse_required_u64(fields, "44", "Price")?,
        _ => 0,
    };
    let symbol = fields.get("55").cloned();
    let transact_time = parse_optional_u64(fields, "60");

    Ok(FixIncomingMessage::NewOrder(NewOrderRequest {
        order_id,
        side,
        quantity,
        price_type,
        price,
        symbol,
        transact_time,
    }))
}

fn parse_cancel_order(fields: &HashMap<String, String>) -> Result<FixIncomingMessage, String> {
    let request_id = parse_optional_u64(fields, "11");
    let orig_order_id = parse_required_u64(fields, "41", "OrigClOrdID")?;
    let symbol = fields.get("55").cloned();

    Ok(FixIncomingMessage::Cancel(CancelOrderRequest {
        request_id,
        orig_order_id,
        symbol,
    }))
}

fn parse_replace_order(fields: &HashMap<String, String>) -> Result<FixIncomingMessage, String> {
    let new_order_id = parse_required_u64(fields, "11", "ClOrdID")?;
    let orig_order_id = parse_required_u64(fields, "41", "OrigClOrdID")?;
    let side = side_from_fix(required_field(fields, "54")?)?;
    let quantity = parse_required_u32(fields, "38", "OrderQty")?;
    let price_type = price_type_from_fix(required_field(fields, "40")?)?;
    let price = match price_type {
        crate::data_types::ORDER_PRICE_TYPE_LIMIT => parse_required_u64(fields, "44", "Price")?,
        _ => 0,
    };
    let symbol = fields.get("55").cloned();
    let transact_time = parse_optional_u64(fields, "60");

    Ok(FixIncomingMessage::Replace(ReplaceOrderRequest {
        new_order_id,
        orig_order_id,
        side,
        quantity,
        price_type,
        price,
        symbol,
        transact_time,
    }))
}

fn parse_status_request(fields: &HashMap<String, String>) -> Result<FixIncomingMessage, String> {
    let request_id = parse_optional_u64(fields, "11");
    let order_id = match parse_optional_u64(fields, "37") {
        Some(value) => value,
        None => parse_required_u64(fields, "11", "ClOrdID")?,
    };
    let symbol = fields.get("55").cloned();

    Ok(FixIncomingMessage::Status(StatusRequest {
        request_id,
        order_id,
        symbol,
    }))
}

fn parse_fix_fields(input: &str) -> HashMap<String, String> {
    let normalized = input.replace('\u{1}', "|");
    let mut fields = HashMap::new();

    for raw in normalized.split('|') {
        if raw.is_empty() {
            continue;
        }
        if let Some((tag, value)) = raw.split_once('=') {
            fields.insert(tag.trim().to_string(), value.trim().to_string());
        }
    }

    fields
}

fn required_field<'a>(fields: &'a HashMap<String, String>, tag: &str) -> Result<&'a str, String> {
    fields
        .get(tag)
        .map(String::as_str)
        .ok_or_else(|| format!("missing required tag {tag}"))
}

fn parse_required_u64(
    fields: &HashMap<String, String>,
    tag: &str,
    field_name: &str,
) -> Result<u64, String> {
    required_field(fields, tag)?
        .parse::<u64>()
        .map_err(|_| format!("{field_name}({tag}) must be a u64 string"))
}

fn parse_required_u32(
    fields: &HashMap<String, String>,
    tag: &str,
    field_name: &str,
) -> Result<u32, String> {
    required_field(fields, tag)?
        .parse::<u32>()
        .map_err(|_| format!("{field_name}({tag}) must be a u32"))
}

fn parse_optional_u64(fields: &HashMap<String, String>, tag: &str) -> Option<u64> {
    fields.get(tag).and_then(|value| value.parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::parse_fix_message;
    use crate::data_types::{ORDER_PRICE_TYPE_LIMIT, ORDER_TYPE_BUY};
    use crate::fix::model::FixIncomingMessage;

    #[test]
    fn parses_new_order_with_pipe_delimiter() {
        let message = parse_fix_message("35=D|11=1001|54=1|38=5|40=2|44=101|55=AAPL|").unwrap();

        match message {
            FixIncomingMessage::NewOrder(order) => {
                assert_eq!(order.order_id, 1001);
                assert_eq!(order.side, ORDER_TYPE_BUY);
                assert_eq!(order.quantity, 5);
                assert_eq!(order.price_type, ORDER_PRICE_TYPE_LIMIT);
                assert_eq!(order.price, 101);
                assert_eq!(order.symbol.as_deref(), Some("AAPL"));
            }
            other => panic!("expected new order, got {other:?}"),
        }
    }

    #[test]
    fn parses_replace_order_with_soh_delimiter() {
        let raw = "35=G\u{1}11=1002\u{1}41=1001\u{1}54=1\u{1}38=8\u{1}40=2\u{1}44=102\u{1}";
        let message = parse_fix_message(raw).unwrap();

        match message {
            FixIncomingMessage::Replace(order) => {
                assert_eq!(order.new_order_id, 1002);
                assert_eq!(order.orig_order_id, 1001);
                assert_eq!(order.quantity, 8);
                assert_eq!(order.price, 102);
            }
            other => panic!("expected replace order, got {other:?}"),
        }
    }

    #[test]
    fn rejects_non_numeric_cl_ord_id() {
        let err = parse_fix_message("35=D|11=abc|54=1|38=5|40=2|44=101|").unwrap_err();
        assert!(err.contains("ClOrdID(11) must be a u64 string"));
    }

    #[test]
    fn parses_status_request() {
        let message = parse_fix_message("35=H|11=5001|37=1001|55=AAPL|").unwrap();

        match message {
            FixIncomingMessage::Status(request) => {
                assert_eq!(request.request_id, Some(5001));
                assert_eq!(request.order_id, 1001);
                assert_eq!(request.symbol.as_deref(), Some("AAPL"));
            }
            other => panic!("expected status request, got {other:?}"),
        }
    }
}
