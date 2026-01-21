use std::time::{Duration, Instant};

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use core::arch::x86_64::_rdtsc;

/// ------------------------------------------------------------
/// High-Resolution Timer (Cross-Platform)
/// ------------------------------------------------------------
/// • x86/x86_64: Uses `rdtsc`
/// • ARM64/AArch64 (Apple Silicon): Uses `cntvct_el0`
/// • Others: Falls back to `Instant::now()`
pub struct HighResolutionCounter {
    start_cycles: u64,
    start_time: Instant,
    tick_ghz: f64,
}

impl HighResolutionCounter {
    /// Start the timer.
    /// Note: `tick_ghz` is the frequency of the counter, not necessarily 
    /// the CPU's boost clock. 
    /// - On Apple Silicon, this is usually 0.024 (24 MHz).
    pub fn start(tick_ghz: f64) -> Self {
        let start_cycles = Self::get_ticks();

        Self {
            start_cycles,
            start_time: Instant::now(),
            tick_ghz,
        }
    }

    #[inline(always)]
    fn get_ticks() -> u64 {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        unsafe { _rdtsc() }

        #[cfg(target_arch = "aarch64")]
        {
            let val: u64;
            unsafe {
                // Read the virtual count register
                std::arch::asm!("mrs {}, cntvct_el0", out(reg) val);
            }
            val
        }

        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
        0
    }

    /// Return elapsed time in **nanoseconds**.
    pub fn ns(&self) -> u128 {
        //0.024
        #[cfg(any(target_arch = "aarch64"))]
        {
            let end = Self::get_ticks();
            let delta_cycles = end.wrapping_sub(self.start_cycles);
            // Convert cycles → nanoseconds (Cycles / GHz)
            //return (delta_cycles as f64 / self.tick_ghz) as u128;
            return (delta_cycles as f64/0.024) as u128;
        }

        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            let end = Self::get_ticks();
            let delta_cycles = end.wrapping_sub(self.start_cycles);
            // Convert cycles → nanoseconds (Cycles / GHz)
            //return (delta_cycles as f64 / self.tick_ghz) as u128;
            return (delta_cycles as f64 / self.tick_ghz) as u128;
        }

        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
        self.start_time.elapsed().as_nanos()
    }

    pub fn us(&self) -> f64 { self.ns() as f64 / 1_000.0 }
    pub fn ms(&self) -> f64 { self.ns() as f64 / 1_000_000.0 }
    pub fn duration(&self) -> Duration { Duration::from_nanos(self.ns() as u64) }
}