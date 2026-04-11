use std::fs::File;
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};


pub struct Stats {
    pub p10: u32, pub p20: u32, pub p30: u32, pub p40: u32,
    pub p50: u32, pub p60: u32, pub p70: u32, pub p80: u32,
    pub p90: u32, pub p95: u32, pub p96: u32, pub p97: u32,
    pub p98: u32, pub p99: u32, pub p999: u32, pub p100: u32,
}



pub fn save_perf_to_file(data: &[u32]) -> std::io::Result<()>  {
    // 当前时间（秒）
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs() as i64;

    // 转换为本地时间
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe {
        libc::localtime_r(&now, &mut tm);
    }

    // YYYY-MM-DD-HH-MM-SS.perf
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

    for v in data {
        writeln!(file, "{}", v)?;
    }

    Ok(())
}

/// 计算统计信息
pub fn calculate_perf(perf_data: &[u32]) -> Option<Stats> {
    if perf_data.is_empty() {
        return None;
    }

    // clone 出一份用于排序（不影响原数据）
    let mut data = perf_data.to_vec();
    data.sort_unstable();

    let len = data.len();

    let get_p = |p: f64| -> u32 {
        // 百分位 → 索引
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


/// 打印纯数据百分位表格
pub fn print_stats_table(s: &Stats) {
    let headers = [
        "P10", "P20", "P50", "P90", 
        "P95", "P96", "P97", "P98", 
        "P99", "P999", "P100"
    ];

    // 宽度计算：16个列 * 8字符 = 128
    let divider = "-".repeat(11*8);

    // 打印表头
    for h in headers {
        print!("{:>8}", h);
    }
    println!();
    println!("{}", divider);

    // 打印数值
    let values = [
        s.p10, s.p20, s.p50, s.p90,
        s.p95, s.p96, s.p97, s.p98, 
        s.p99, s.p999, s.p100
    ];
    for v in values {
        print!("{:>8}", v);
    }
    println!();
    println!("{}", divider);
}