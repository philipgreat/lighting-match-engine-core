use lighting_match_engine_core::config::get_config;
use lighting_match_engine_core::cpu_affinity::set_core;
use lighting_match_engine_core::data_types::EngineState;
use lighting_match_engine_core::high_resolution_timer::HighResolutionTimer;
use lighting_match_engine_core::matching_engine::{make_benchmark_order, tag_to_u16_array};
use lighting_match_engine_core::number_tool::Separatable;
use lighting_match_engine_core::perf_stats;
use lighting_match_engine_core::text_output_tool::{print_centered_line, print_separator, show_result};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "============= BUILD at {}  by {}@{} ====================\n",
        env!("BUILD_TIME"),
        env!("BUILD_USER"),
        env!("BUILD_HOSTNAME")
    );

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
    println!(
        "  Test order book size: {} bids and {}  asks repectively",
        test_order_book_size, test_order_book_size
    );

    print_separator(100);

    set_core(0);

    let instance_tag_bytes = tag_to_u16_array(&tag_string);
    
    // 3. Initialize Engine State
    let mut engine_state = EngineState::new(instance_tag_bytes, prod_id);
    engine_state.load_sample_test_book(test_order_book_size);

    let count = 10000u64;
    let timer = HighResolutionTimer::start();

    let start = timer.ns() as u64;

    let mut perf_data = Vec::with_capacity(count as usize * 2);

    for i in 0..count {
        let new_order_buy = make_benchmark_order(7, 1_000_000_000 + i, true, 1);

        engine_state.match_order(new_order_buy);
        let new_order_sell = make_benchmark_order(7, 2_000_000_000 + i + 1, false, 1);
        engine_state.match_order(new_order_sell);
    }

    for i in 0..count {
        let new_order_buy = make_benchmark_order(7, 1_000_000_000 + i, true, 1);

        engine_state.match_order(new_order_buy);

        perf_data.push(engine_state.order_book.match_result.time_per_order_execution() as u32);

        let new_order_sell = make_benchmark_order(7, 2_000_000_000 + i + 1, false, 9);
        engine_state.match_order(new_order_sell);
        perf_data.push(engine_state.order_book.match_result.time_per_order_execution() as u32);
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
    let last_result = engine_state.order_book.match_result;
    //println!("result {:?}", engine_state.order_book.match_result);

    print_centered_line("Last match result", '-', 80);
    if last_result.total_count() > 0 {
        println!(
            "\nTotal time: {}ns for {} order executions, avarage {}ns per order execution\n",
            last_result.total_time(),
            last_result.total_count(),
            last_result.total_time() / last_result.total_count() as u64
        );
    }

    show_result(last_result);

    if let Some(stats) = lighting_match_engine_core::perf_stats::calculate_perf(&perf_data) {
        lighting_match_engine_core::perf_stats::print_stats_table(&stats);
    } else {
        println!("数据为空，无法统计");
    }
    print_separator(100);

    perf_stats::save_perf_to_file(&perf_data)?;
    // println!("{:?} ns ",engine_state.order_book.match_result.total_time());

    // engine_state.order_book.match_result.order_execution_list.iter().for_each(|oe|{
    //     println!("{:?}",oe);
    // });

    Ok(())
}
