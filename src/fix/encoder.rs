use super::model::{build_exec_id, ExecutionReport, ExecutionType, OrderStatusReport, RejectReport};

pub fn encode_execution_report(report: &ExecutionReport) -> String {
    let exec_type = match report.exec_type {
        ExecutionType::New => "0",
        ExecutionType::Trade => "F",
        ExecutionType::Canceled => "4",
        ExecutionType::Rejected => "8",
    };

    let mut fields = vec![
        ("35", "8".to_string()),
        ("11", report.order_id.to_string()),
        ("17", report.exec_id.clone()),
        ("150", exec_type.to_string()),
        ("39", report.ord_status.to_string()),
        ("14", report.cum_qty.to_string()),
        ("151", report.leaves_qty.to_string()),
        ("32", report.last_qty.to_string()),
        ("31", report.last_px.to_string()),
    ];

    if let Some(symbol) = &report.symbol {
        fields.push(("55", symbol.clone()));
    }
    if let Some(text) = &report.text {
        fields.push(("58", text.clone()));
    }

    render(fields)
}

pub fn encode_cancel_report(order_id: u64, symbol: Option<String>, leaves_qty: u32) -> String {
    let mut fields = vec![
        ("35", "8".to_string()),
        ("11", order_id.to_string()),
        ("17", build_exec_id(order_id, &ExecutionType::Canceled, 0)),
        ("150", "4".to_string()),
        ("39", "4".to_string()),
        ("14", "0".to_string()),
        ("151", leaves_qty.to_string()),
        ("32", "0".to_string()),
        ("31", "0".to_string()),
    ];

    if let Some(symbol) = symbol {
        fields.push(("55", symbol));
    }

    render(fields)
}

pub fn encode_order_status_report(report: &OrderStatusReport) -> String {
    let wrapped = ExecutionReport {
        exec_id: build_exec_id(report.order_id, &report.exec_type, 0),
        order_id: report.order_id,
        exec_type: report.exec_type.clone(),
        ord_status: report.ord_status,
        last_qty: report.last_qty,
        last_px: report.last_px,
        leaves_qty: report.leaves_qty,
        cum_qty: report.cum_qty,
        text: None,
        symbol: report.symbol.clone(),
    };

    encode_execution_report(&wrapped)
}

pub fn encode_reject(report: &RejectReport) -> String {
    let mut fields = vec![
        ("35", "8".to_string()),
        ("17", report.exec_id.clone()),
        ("150", "8".to_string()),
        ("39", "8".to_string()),
        ("58", report.text.clone()),
    ];

    if let Some(order_id) = report.order_id {
        fields.push(("11", order_id.to_string()));
    }

    render(fields)
}

fn render(fields: Vec<(&str, String)>) -> String {
    let mut out = String::new();
    for (tag, value) in fields {
        out.push_str(tag);
        out.push('=');
        out.push_str(&value);
        out.push('|');
    }
    out
}
