pub trait Clock {
    fn now_ns(&self) -> u64;
}

pub trait Stopwatch {
    fn elapsed_ns(&self) -> u64;
}
