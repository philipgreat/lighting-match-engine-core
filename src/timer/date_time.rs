use std::time::{SystemTime, UNIX_EPOCH};

pub fn current_timestamp() -> u64 {
    //time::Instant::now().elapsed().as_nanos() as u64
    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("fail")
        .as_nanos() as u64;
    now_nanos
}
