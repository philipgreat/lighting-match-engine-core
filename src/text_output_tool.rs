
use crate::data_types::{MatchResult};

pub fn print_separator(eq_len: usize) {
    println!("\n{}\n", "=".repeat(eq_len));
}
pub fn show_result(result: MatchResult) {
    if result.order_execution_list.is_empty() {
        return;
    }

    let time_per_order_execution =
        result.total_time() as usize / result.order_execution_list.len();

    // column widths
    const W_TYPE: usize = 24;
    const W_PRODUCT: usize = 8;
    const W_PRICE: usize = 8;
    const W_QTY: usize = 6;
    const W_BUY: usize = 14;
    const W_SELL: usize = 14;
    const W_LAT: usize = 10;

    let header = format!(
        "{:<W_TYPE$} {:<W_PRODUCT$} {:<W_PRICE$} {:<W_QTY$} {:<W_BUY$} {:<W_SELL$} {:<W_LAT$}",
        "MSG Type",
        "Product",
        "Price",
        "Qty",
        "BuyOrderID",
        "SellOrderID",
        "Lat(ns)",
        W_TYPE = W_TYPE,
        W_PRODUCT = W_PRODUCT,
        W_PRICE = W_PRICE,
        W_QTY = W_QTY,
        W_BUY = W_BUY,
        W_SELL = W_SELL,
        W_LAT = W_LAT,
    );

    let sep = "-".repeat(header.len());

    for (i, o) in result.order_execution_list.iter().enumerate() {
        // print header every 10 rows
        if i % 10 == 0 {
            println!("{}", sep);
            println!("{}", header);
            println!("{}", sep);
        }

        println!(
            "{:<W_TYPE$} {:<W_PRODUCT$} {:<W_PRICE$} {:<W_QTY$} {:<W_BUY$} {:<W_SELL$} {:<W_LAT$}",
            "🔥 ORDER EXECUTION",
            o.product_id,
            o.price,
            o.quantity,
            o.buy_order_id,
            o.sell_order_id,
            time_per_order_execution,
            W_TYPE = W_TYPE,
            W_PRODUCT = W_PRODUCT,
            W_PRICE = W_PRICE,
            W_QTY = W_QTY,
            W_BUY = W_BUY,
            W_SELL = W_SELL,
            W_LAT = W_LAT,
        );
    }

    println!("{}", sep);
}
pub fn print_centered_line(text: &str, fill: char, total_width: usize) {
    let text_len = text.len();

    if total_width <= text_len {
        println!("{}", text);
        return;
    }

    let padding = total_width - text_len;
    let left = padding / 2;
    let right = padding - left;

    println!(
        "{}{}{}",
        fill.to_string().repeat(left),
        text,
        fill.to_string().repeat(right)
    );
}
