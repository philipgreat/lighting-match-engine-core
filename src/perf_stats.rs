/// 性能统计模块

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

/// 计算给定 Vec<u32> 的百分位数统计
/// 注意：该函数会消耗/修改传入的 Vec（为了排序）
pub fn calculate_perf(mut data: Vec<u32>) -> Option<Stats> {
    if data.is_empty() {
        return None;
    }

    // 1. 首先进行排序，这是计算百分位数的前提
    data.sort_unstable();

    let len = data.len();

    // 辅助闭包：根据百分比计算对应的元素
    // 使用最近邻插值法
    let get_p = |p: f64| -> u32 {
        if p >= 100.0 {
            return data[len - 1];
        }
        // 计算索引：p/100 * len
        let idx = (p / 100.0 * len as f64).ceil() as usize;
        // 边界检查，确保不越界，且至少取到第0个元素
        let pos = idx.saturating_sub(1).min(len - 1);
        data[pos]
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

/// 打印统计结果的辅助函数
pub fn print_stats(s: &Stats) {
    println!("--- 性能百分位数统计 ---");
    println!("P10 - P40:  [{}, {}, {}, {}]", s.p10, s.p20, s.p30, s.p40);
    println!("P50 (中位数): {}", s.p50);
    println!("P60 - P90:  [{}, {}, {}, {}]", s.p60, s.p70, s.p80, s.p90);
    println!("--- 高分位值 ---");
    println!("P95:  {} | P96:  {} | P97:  {}", s.p95, s.p96, s.p97);
    println!("P98:  {} | P99:  {} | P999: {}", s.p98, s.p99, s.p999);
    println!("P100(最大值): {}", s.p100);
}
