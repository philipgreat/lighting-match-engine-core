use crate::data_types::{
    MatchResult, Order, ORDER_PRICE_TYPE_LIMIT, ORDER_PRICE_TYPE_MARKET, ORDER_TYPE_BUY,
    ORDER_TYPE_SELL,
};

#[derive(Debug, Clone)]
pub enum FixIncomingMessage {
    NewOrder(NewOrderRequest),
    Cancel(CancelOrderRequest),
    Replace(ReplaceOrderRequest),
    Status(StatusRequest),
}

#[derive(Debug, Clone)]
pub struct NewOrderRequest {
    pub order_id: u64,
    pub side: u8,
    pub quantity: u32,
    pub price_type: u8,
    pub price: u64,
    pub symbol: Option<String>,
    pub transact_time: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct CancelOrderRequest {
    pub request_id: Option<u64>,
    pub orig_order_id: u64,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReplaceOrderRequest {
    pub new_order_id: u64,
    pub orig_order_id: u64,
    pub side: u8,
    pub quantity: u32,
    pub price_type: u8,
    pub price: u64,
    pub symbol: Option<String>,
    pub transact_time: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct StatusRequest {
    pub request_id: Option<u64>,
    pub order_id: u64,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ExecutionType {
    New,
    Trade,
    Canceled,
    Rejected,
}

#[derive(Debug, Clone)]
pub struct ExecutionReport {
    pub exec_id: String,
    pub order_id: u64,
    pub exec_type: ExecutionType,
    pub ord_status: &'static str,
    pub last_qty: u32,
    pub last_px: u64,
    pub leaves_qty: u32,
    pub cum_qty: u32,
    pub text: Option<String>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RejectReport {
    pub exec_id: String,
    pub order_id: Option<u64>,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct OrderStatusReport {
    pub order_id: u64,
    pub symbol: Option<String>,
    pub cum_qty: u32,
    pub leaves_qty: u32,
    pub last_qty: u32,
    pub last_px: u64,
    pub ord_status: &'static str,
    pub exec_type: ExecutionType,
}

impl NewOrderRequest {
    pub fn into_order(self, product_id: u16, submit_time: u64) -> Order {
        Order {
            product_id,
            order_side: self.side,
            price_type: self.price_type,
            quantity: self.quantity,
            order_id: self.order_id,
            price: self.price,
            submit_time: self.transact_time.unwrap_or(submit_time),
            expire_time: 0,
            _padding: [0u8; 24],
        }
    }
}

impl ReplaceOrderRequest {
    pub fn into_order(self, product_id: u16, submit_time: u64) -> Order {
        Order {
            product_id,
            order_side: self.side,
            price_type: self.price_type,
            quantity: self.quantity,
            order_id: self.new_order_id,
            price: self.price,
            submit_time: self.transact_time.unwrap_or(submit_time),
            expire_time: 0,
            _padding: [0u8; 24],
        }
    }
}

pub fn side_from_fix(value: &str) -> Result<u8, String> {
    match value {
        "1" => Ok(ORDER_TYPE_BUY),
        "2" => Ok(ORDER_TYPE_SELL),
        _ => Err(format!("unsupported Side(54): {value}")),
    }
}

pub fn price_type_from_fix(value: &str) -> Result<u8, String> {
    match value {
        "1" => Ok(ORDER_PRICE_TYPE_MARKET),
        "2" => Ok(ORDER_PRICE_TYPE_LIMIT),
        _ => Err(format!("unsupported OrdType(40): {value}")),
    }
}

pub fn build_execution_reports(
    request: &NewOrderRequest,
    resting_qty: u32,
    result: &MatchResult,
) -> Vec<ExecutionReport> {
    let mut reports = Vec::new();
    let mut cum_qty = 0u32;

    if result.order_execution_list.is_empty() {
        reports.push(ExecutionReport {
            exec_id: build_exec_id(request.order_id, &ExecutionType::New, 0),
            order_id: request.order_id,
            exec_type: ExecutionType::New,
            ord_status: if resting_qty == 0 { "2" } else { "0" },
            last_qty: 0,
            last_px: request.price,
            leaves_qty: resting_qty,
            cum_qty: 0,
            text: None,
            symbol: request.symbol.clone(),
        });
        return reports;
    }

    for (index, execution) in result.order_execution_list.iter().enumerate() {
        let last_qty = execution.quantity;
        cum_qty = cum_qty.saturating_add(last_qty);
        let leaves_qty = request.quantity.saturating_sub(cum_qty);
        let ord_status = if leaves_qty == 0 { "2" } else { "1" };

        reports.push(ExecutionReport {
            exec_id: build_exec_id(request.order_id, &ExecutionType::Trade, index),
            order_id: request.order_id,
            exec_type: ExecutionType::Trade,
            ord_status,
            last_qty,
            last_px: execution.price,
            leaves_qty,
            cum_qty,
            text: None,
            symbol: request.symbol.clone(),
        });
    }

    reports
}

pub fn build_exec_id(order_id: u64, exec_type: &ExecutionType, sequence: usize) -> String {
    let suffix = match exec_type {
        ExecutionType::New => "NEW",
        ExecutionType::Trade => "TRD",
        ExecutionType::Canceled => "CXL",
        ExecutionType::Rejected => "REJ",
    };

    format!("{order_id}-{suffix}-{sequence}")
}
