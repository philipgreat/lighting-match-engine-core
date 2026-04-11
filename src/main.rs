mod config;
mod orderbook;
mod protocol;
mod stats;
mod system;
mod timer;
mod types;
mod utils;

use std::process::ExitCode;

use crate::protocol::{format_order_submit_error_cli, serialize_order_book_error_reply, OrderBookErrorReply};
use crate::utils::Separatable;
use crate::types::{EngineState, OrderBook, OrderFlags, OrderRequest, OrderSide, PriceType};
use crate::stats::{print_centered_line, print_separator, show_result};
use crate::orderbook::build_order_book;
use crate::system::set_core;

use config::get_config;
use crate::timer::HighResolutionTimer;

fn tag_to_u16_array(tag: &str) -> [u8; 16] {
    let mut tag_array = [0u8; 16];
    let bytes = tag.as_bytes();
    let len = std::cmp::min(bytes.len(), 16);
    tag_array[..len].copy_from_slice(&bytes[..len]);
    tag_array
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", err);
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "============= BUILD at {}  by {}@{} ====================\n",
        env!("BUILD_TIME"),
        env!("BUILD_USER"),
        env!("BUILD_HOSTNAME")
    );

    println!("Starting Lighting Match Engine Core...");

    // 1. Get configuration
    let app_config = match get_config() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Configuration Error: {}", e);
            eprintln!(
                "Usage: --name <tag_16_chars_max> --prodid <u16> [--test-order-book-size 10k] [--order-book dense|sparse] [--tick N] [--base-price N] [--max-levels N] [--trade-cap N]"
            );
            return Err(e.into());
        }
    };

    println!("Configuration Loaded:");
    println!("  Instance Tag: {}", app_config.instance_name);
    println!("  Product ID: {}", app_config.product_id);
    println!("  Order Book: {:?}", app_config.order_book.kind);
    println!(
        "  Order Book Config: tick={} base_price={} max_levels={} trade_cap={}",
        app_config.order_book.tick,
        app_config.order_book.base_price,
        app_config.order_book.max_levels,
        app_config.order_book.trade_cap
    );
    println!(
        "  Test order book size: {} bids and {}  asks repectively",
        app_config.test_order_book_size, app_config.test_order_book_size
    );

    print_separator(100);

    set_core(0);

    let instance_tag_bytes = tag_to_u16_array(&app_config.instance_name);
    let order_book = build_order_book(app_config.order_book);
    
    // 3. Initialize Engine State
    let mut engine_state = EngineState::new(instance_tag_bytes, app_config.product_id, order_book);
    if let Err(err) = engine_state.load_sample_test_book(app_config.test_order_book_size) {
        eprintln!("{}", format_order_submit_error_cli(&err));
        let reply = OrderBookErrorReply::from_submit_error(instance_tag_bytes, &err);
        let _encoded_reply = serialize_order_book_error_reply(&reply);
        return Err(Box::new(err));
    }

    let count = 1000u64;
    let timer = HighResolutionTimer::start();

    let start = timer.ns() as u64;

    let mut perf_data = Vec::with_capacity(count as usize * 2);

    for i in 0..count {
        let new_order_buy = OrderRequest {
            product_id: 7,
            side: OrderSide::Buy,
            price: 10000,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            quantity: 1,
            order_id: 1_000_000_000 + i,
            submit_time: 100,
            expire_time: 0,
            _padding: [0u8; 24],
        };
        
        if let Err(err) = engine_state.match_order(new_order_buy) {
            eprintln!("{}", format_order_submit_error_cli(&err));
            let reply = OrderBookErrorReply::from_submit_error(instance_tag_bytes, &err);
            let _encoded_reply = serialize_order_book_error_reply(&reply);
            return Err(Box::new(err));
        }

        //perf_data.push(engine_state.order_book.match_result.time_per_order_execution() as u32);

        let new_order_sell = OrderRequest {
            product_id: 7,
            side: OrderSide::Sell,
            price: 1,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            quantity: 1,
            order_id: 2_000_000_000 + i + 1,
            submit_time: 2_000_000_000 + i + 1,
            expire_time: 0,
            _padding: [0u8; 24],
        };
        if let Err(err) = engine_state.match_order(new_order_sell) {
            eprintln!("{}", format_order_submit_error_cli(&err));
            let reply = OrderBookErrorReply::from_submit_error(instance_tag_bytes, &err);
            let _encoded_reply = serialize_order_book_error_reply(&reply);
            return Err(Box::new(err));
        }

        //perf_data.push(engine_state.order_book.match_result.time_per_order_execution() as u32);
    }

    for i in 0..count {
        let new_order_buy = OrderRequest {
            product_id: 7,
            side: OrderSide::Buy,
            price: 10000,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            quantity: 1,
            order_id: 1_000_000_000 + i,
            submit_time: 100,
            expire_time: 0,
            _padding: [0u8; 24],
        };

        if let Err(err) = engine_state.match_order(new_order_buy) {
            eprintln!("{}", format_order_submit_error_cli(&err));
            let reply = OrderBookErrorReply::from_submit_error(instance_tag_bytes, &err);
            let _encoded_reply = serialize_order_book_error_reply(&reply);
            return Err(Box::new(err));
        }

        perf_data.push(engine_state.order_book.last_outcome().time_per_trade() as u32);

        let new_order_sell = OrderRequest {
            product_id: 7,
            side: OrderSide::Sell,
            price: 1,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            quantity: 9,
            order_id: 2_000_000_000 + i + 1,
            submit_time: 2_000_000_000 + i + 1,
            expire_time: 0,
            _padding: [0u8; 24],
        };
        if let Err(err) = engine_state.match_order(new_order_sell) {
            eprintln!("{}", format_order_submit_error_cli(&err));
            let reply = OrderBookErrorReply::from_submit_error(instance_tag_bytes, &err);
            let _encoded_reply = serialize_order_book_error_reply(&reply);
            return Err(Box::new(err));
        }
        perf_data.push(engine_state.order_book.last_outcome().time_per_trade() as u32);
    }
    let end = timer.ns() as u64;
    println!(
        "Elapsed: {:>15} ns for {} match results.",
        (end - start).separated_string(),
        (2 * count).separated_string()
    );
    println!(
        "Speed  : {:>15} match results/sec.\n",
        ((1_000_000_000) * (2 * count) / (end - start)).separated_string()
    );
    let last_result = engine_state.order_book.last_outcome().clone();
    //println!("result {:?}", engine_state.order_book.last_outcome());

    print_centered_line("Last match result", '-', 80);
    if last_result.total_count() > 0 {
        println!(
            "\nTotal time: {}ns for {} trades, avarage {}ns per trade\n",
            last_result.total_time(),
            last_result.total_count(),
            last_result.total_time() / last_result.total_count() as u64
        );
    }

    show_result(last_result);

    if let Some(stats) = stats::calculate_perf(&perf_data) {
        stats::print_stats_table(&stats);
    } else {
        println!("数据为空，无法统计");
    }
    print_separator(100);

    stats::save_perf_to_file(&perf_data)?;
    // println!("{:?} ns ",engine_state.order_book.last_outcome().total_time());

    // engine_state.order_book.last_outcome().trades.iter().for_each(|oe|{
    //     println!("{:?}",oe);
    // });

    Ok(())
}
