use crate::utils::parse_human_readable_u32;

#[derive(Debug, Clone, Copy)]
pub enum OrderBookKind {
    Dense,
    Sparse,
}

#[derive(Debug, Clone, Copy)]
pub struct OrderBookConfig {
    pub kind: OrderBookKind,
    pub tick: u64,
    pub base_price: u64,
    pub max_levels: usize,
    pub trade_cap: usize,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub instance_name: String,
    pub product_id: u16,
    pub test_order_book_size: u32,
    pub order_book: OrderBookConfig,
    pub run_call_auction_benchmark: bool,
    pub benchmark_only: bool,
}

fn validate_order_book_config(config: OrderBookConfig) -> Result<OrderBookConfig, String> {
    if config.tick == 0 {
        return Err("Invalid order book config: tick must be greater than 0.".to_string());
    }

    if config.max_levels == 0 {
        return Err("Invalid order book config: max_levels must be greater than 0.".to_string());
    }

    if config.trade_cap == 0 {
        return Err("Invalid order book config: trade_cap must be greater than 0.".to_string());
    }

    match config.kind {
        OrderBookKind::Dense => {
            if config.max_levels > isize::MAX as usize {
                return Err(format!(
                    "Invalid dense order book config: max_levels {} exceeds isize::MAX {}.",
                    config.max_levels,
                    isize::MAX
                ));
            }

            let level_span = (config.max_levels as u64).saturating_sub(1);
            let max_offset = config
                .tick
                .checked_mul(level_span)
                .ok_or_else(|| {
                    "Invalid dense order book config: tick * (max_levels - 1) overflowed."
                        .to_string()
                })?;

            config
                .base_price
                .checked_add(max_offset)
                .ok_or_else(|| {
                    "Invalid dense order book config: base_price + price span overflowed."
                        .to_string()
                })?;
        }
        OrderBookKind::Sparse => {}
    }

    Ok(config)
}

fn parse_u64_arg(name: &str, value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("Invalid {} '{}'. Must be a valid u64.", name, value))
}

fn parse_usize_arg(name: &str, value: &str) -> Result<usize, String> {
    let parsed = parse_human_readable_u32(value)
        .map_err(|e| format!("Invalid {} '{}': {}", name, value, e))?;
    Ok(parsed as usize)
}

pub fn get_config() -> Result<AppConfig, String> {
    let args: Vec<String> = std::env::args().collect();
    let mut instance_name = None;
    let mut product_id = None;
    let mut test_order_book_size_str = None;
    let mut order_book_kind = None;
    let mut order_book_tick = None;
    let mut order_book_base_price = None;
    let mut order_book_max_levels = None;
    let mut order_book_trade_cap = None;
    let mut run_call_auction_benchmark = None;
    let mut benchmark_only = None;

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
            "--order-book" => {
                if i + 1 < args.len() {
                    order_book_kind = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--tick" => {
                if i + 1 < args.len() {
                    order_book_tick = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--base-price" => {
                if i + 1 < args.len() {
                    order_book_base_price = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--max-levels" => {
                if i + 1 < args.len() {
                    order_book_max_levels = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--trade-cap" => {
                if i + 1 < args.len() {
                    order_book_trade_cap = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--bench-call-auction" => {
                run_call_auction_benchmark = Some(true);
            }
            "--bench-call-auction-only" => {
                run_call_auction_benchmark = Some(true);
                benchmark_only = Some(true);
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

    let kind = match order_book_kind
        .or_else(|| std::env::var("ORDER_BOOK").ok())
        .as_deref()
        .unwrap_or("dense")
    {
        "dense" => OrderBookKind::Dense,
        "sparse" => OrderBookKind::Sparse,
        other => {
            return Err(format!(
                "Invalid --order-book '{}'. Expected 'dense' or 'sparse'.",
                other
            ))
        }
    };

    let tick = parse_u64_arg(
        "--tick",
        order_book_tick
            .or_else(|| std::env::var("ORDER_BOOK_TICK").ok())
            .as_deref()
            .unwrap_or("1"),
    )?;

    let base_price = parse_u64_arg(
        "--base-price",
        order_book_base_price
            .or_else(|| std::env::var("ORDER_BOOK_BASE_PRICE").ok())
            .as_deref()
            .unwrap_or("1"),
    )?;

    let max_levels = parse_usize_arg(
        "--max-levels",
        order_book_max_levels
            .or_else(|| std::env::var("ORDER_BOOK_MAX_LEVELS").ok())
            .as_deref()
            .unwrap_or("1000000"),
    )?;

    let trade_cap = parse_usize_arg(
        "--trade-cap",
        order_book_trade_cap
            .or_else(|| std::env::var("ORDER_BOOK_TRADE_CAP").ok())
            .as_deref()
            .unwrap_or("100"),
    )?;

    let order_book = validate_order_book_config(OrderBookConfig {
        kind,
        tick,
        base_price,
        max_levels,
        trade_cap,
    })?;

    Ok(AppConfig {
        instance_name: tag_string,
        product_id: prod_id,
        test_order_book_size,
        order_book,
        run_call_auction_benchmark: run_call_auction_benchmark
            .or_else(|| {
                std::env::var("BENCH_CALL_AUCTION")
                    .ok()
                    .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            })
            .unwrap_or(false),
        benchmark_only: benchmark_only
            .or_else(|| {
                std::env::var("BENCH_CALL_AUCTION_ONLY")
                    .ok()
                    .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            })
            .unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dense_config() -> OrderBookConfig {
        OrderBookConfig {
            kind: OrderBookKind::Dense,
            tick: 100,
            base_price: 1,
            max_levels: 1_000,
            trade_cap: 100,
        }
    }

    #[test]
    fn rejects_zero_tick() {
        let err = validate_order_book_config(OrderBookConfig {
            tick: 0,
            ..dense_config()
        })
        .unwrap_err();

        assert!(err.contains("tick must be greater than 0"));
    }

    #[test]
    fn rejects_zero_max_levels() {
        let err = validate_order_book_config(OrderBookConfig {
            max_levels: 0,
            ..dense_config()
        })
        .unwrap_err();

        assert!(err.contains("max_levels must be greater than 0"));
    }

    #[test]
    fn rejects_zero_trade_cap() {
        let err = validate_order_book_config(OrderBookConfig {
            trade_cap: 0,
            ..dense_config()
        })
        .unwrap_err();

        assert!(err.contains("trade_cap must be greater than 0"));
    }

    #[test]
    fn rejects_dense_tick_span_overflow() {
        let err = validate_order_book_config(OrderBookConfig {
            tick: u64::MAX,
            max_levels: 3,
            ..dense_config()
        })
        .unwrap_err();

        assert!(err.contains("tick * (max_levels - 1) overflowed"));
    }

    #[test]
    fn rejects_dense_base_price_plus_span_overflow() {
        let err = validate_order_book_config(OrderBookConfig {
            tick: 2,
            base_price: u64::MAX,
            max_levels: 2,
            ..dense_config()
        })
        .unwrap_err();

        assert!(err.contains("base_price + price span overflowed"));
    }

    #[test]
    fn allows_sparse_config_without_dense_span_checks() {
        let config = validate_order_book_config(OrderBookConfig {
            kind: OrderBookKind::Sparse,
            tick: u64::MAX,
            base_price: u64::MAX,
            max_levels: 3,
            trade_cap: 1,
        })
        .unwrap();

        assert!(matches!(config.kind, OrderBookKind::Sparse));
    }

    #[test]
    fn parse_usize_arg_supports_human_readable_units() {
        let parsed = parse_usize_arg("--max-levels", "10k").unwrap();
        assert_eq!(parsed, 10_000);
    }
}
