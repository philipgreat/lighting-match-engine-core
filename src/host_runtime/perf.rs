use std::fs::File;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Stats {
    pub p10: u32,
    pub p20: u32,
    pub p30: u32,
    pub p40: u32,
    pub p50: u32,
    pub p60: u32,
    pub p70: u32,
    pub p80: u32,
    pub p90: u32,
    pub p95: u32,
    pub p96: u32,
    pub p97: u32,
    pub p98: u32,
    pub p99: u32,
    pub p999: u32,
    pub p100: u32,
}

pub fn save_perf_to_file(data: &[u32]) -> std::io::Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs() as i64;

    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe {
        libc::localtime_r(&now, &mut tm);
    }

    let filename = format!(
        "perf-data/{:04}-{:02}-{:02}-{:02}-{:02}-{:02}.perf",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec,
    );

    let mut file = File::create(&filename)?;
    for value in data {
        writeln!(file, "{}", value)?;
    }

    Ok(())
}

pub fn calculate_perf(perf_data: &[u32]) -> Option<Stats> {
    if perf_data.is_empty() {
        return None;
    }

    let mut data = perf_data.to_vec();
    data.sort_unstable();

    let len = data.len();
    let get_p = |p: f64| -> u32 {
        let idx = ((p / 100.0) * len as f64).ceil() as usize;
        data[idx.saturating_sub(1).min(len - 1)]
    };

    Some(Stats {
        p10: get_p(10.0),
        p20: get_p(20.0),
        p30: get_p(30.0),
        p40: get_p(40.0),
        p50: get_p(50.0),
        p60: get_p(60.0),
        p70: get_p(70.0),
        p80: get_p(80.0),
        p90: get_p(90.0),
        p95: get_p(95.0),
        p96: get_p(96.0),
        p97: get_p(97.0),
        p98: get_p(98.0),
        p99: get_p(99.0),
        p999: get_p(99.9),
        p100: get_p(100.0),
    })
}

pub fn print_stats_table(stats: &Stats) {
    let headers = ["P10", "P20", "P50", "P90", "P95", "P96", "P97", "P98", "P99", "P999", "P100"];
    let divider = "-".repeat(11 * 8);

    for header in headers {
        print!("{:>8}", header);
    }
    println!();
    println!("{}", divider);

    let values = [
        stats.p10, stats.p20, stats.p50, stats.p90, stats.p95, stats.p96, stats.p97, stats.p98,
        stats.p99, stats.p999, stats.p100,
    ];
    for value in values {
        print!("{:>8}", value);
    }
    println!();
    println!("{}", divider);
}
