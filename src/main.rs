

mod data_types;
mod date_time_tool;
mod engine_state;
mod high_resolution_timer;
mod message_codec;
mod number_tool;
mod continuous_order_book;
mod call_auction_pool;
mod text_output_tool;
mod cpu_affinity;
mod config;
mod perf_stats;


use data_types::{EngineState,ORDER_TYPE_BUY, 
    ORDER_TYPE_SELL,
    ORDER_PRICE_TYPE_LIMIT};

use text_output_tool::{print_centered_line,print_separator,show_result};

use cpu_affinity::set_core;

use config::get_config;
use perf_stats::calculate_perf;
use perf_stats::print_stats;

use crate::{data_types::Order, high_resolution_timer::HighResolutionTimer};



fn tag_to_u16_array(tag: &str) -> [u8; 16] {
    let mut tag_array = [0u8; 16];
    let bytes = tag.as_bytes();
    let len = std::cmp::min(bytes.len(), 16);
    tag_array[..len].copy_from_slice(&bytes[..len]);
    tag_array
}



 fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Lighting Match Engine Core...");

    // 1. Get configuration
    let (tag_string, prod_id, test_order_book_size) = match get_config() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Configuration Error: {}", e);
            eprintln!(
                "Usage: --name <tag_16_chars_max> --prodid <u16> [--test-order-book-size 10k]"
            );
            return Err(e.into());
        }
    };


    println!("Configuration Loaded:");
    println!("  Instance Tag: {}", tag_string);
    println!("  Product ID: {}", prod_id);
    println!("  Test order book size: {} bids and {}  asks pectively", test_order_book_size, test_order_book_size);
    
    
    print_separator(100);
    

    set_core(1);

    let instance_tag_bytes = tag_to_u16_array(&tag_string);

    // 3. Initialize Engine State
    let mut engine_state = EngineState::new(instance_tag_bytes, prod_id);
    engine_state.load_sample_test_book(test_order_book_size);

    let count = 1000u64;
    let timer = HighResolutionTimer::start();

    let start = timer.ns() as u64;
    
    let mut perf_data = Vec::with_capacity(count as usize *2);

    for i in 0..count {

        let  new_order_buy = Order{
            product_id: 7 ,
            order_type: ORDER_TYPE_BUY,
            price:100000000000,
            price_type: ORDER_PRICE_TYPE_LIMIT,
            quantity:5,
            order_id: 1_000_000_000 + i,
            submit_time:100,
            expire_time:0,

        };
        

        engine_state.match_order(new_order_buy);
        perf_data.push(engine_state.continuous_order_book.match_result.time_per_trade() as u32);

        let new_order_sell = Order{
            product_id: 7 ,
            order_type: ORDER_TYPE_SELL,
            price:1,
            price_type: ORDER_PRICE_TYPE_LIMIT,
            quantity:9,
            order_id: 2_000_000_000+i+1,
            submit_time:2_000_000_000+i+1,
            expire_time:0,

        };
        engine_state.match_order(new_order_sell);
        perf_data.push(engine_state.continuous_order_book.match_result.time_per_trade() as u32);


    }
    let end = timer.ns() as u64;
    println!("Time consumed {}ns for {} match requests.", (end-start),2*count);
    println!("Speed: {} match results per second.\n", ( (1_000_000_000)*(2*count ) ) /(end-start));
    let last_result = engine_state.continuous_order_book.match_result;
    //println!("result {:?}", engine_state.continuous_order_book.match_result);
    
    print_centered_line("Last match result",'-',80);
    if last_result.total_count()>0 {
            println!("\nTotal time: {}ns for {} order executions, avarage {}ns per order execution\n", 
        last_result.total_time(), 
        last_result.total_count(),
        last_result.total_time() / last_result.total_count() as u64);
    }


    show_result(last_result);
    
    if let Some(stats) = perf_stats::calculate_perf(perf_data) {
        perf_stats::print_stats(&stats);
    } else {
        println!("数据为空，无法统计");
    }
    print_separator(100);
    // println!("{:?} ns ",engine_state.continuous_order_book.match_result.total_time());

    // engine_state.continuous_order_book.match_result.order_execution_list.iter().for_each(|oe|{
    //     println!("{:?}",oe);
    // });

   
    Ok(())
}


