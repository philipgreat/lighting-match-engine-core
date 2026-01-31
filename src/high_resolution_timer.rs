

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use core::arch::x86_64::{_rdtsc, _mm_lfence};

/// ------------------------------------------------------------
/// High-Resolution Timer (Cross-Platform)
/// ------------------------------------------------------------
/// • x86_64 (Linux/Mac): Uses fenced `rdtsc` for precision
/// • ARM64 (Apple Silicon): Uses `cntvct_el0` (Fixed 24MHz)
/// • Others: Falls back to `Instant::now()`
#[derive(Debug)]
pub struct HighResolutionTimer {
    start_cycles: u64,
    start_time: u64,
    tick_hz: u64,
}

impl HighResolutionTimer {
    /// Start the timer.
    /// - On Apple Silicon: tick_ghz is ignored (internally uses 0.024)
    /// - On Linux/x86: Pass the calibrated TSC frequency (e.g., 2.4)
    pub fn start(tick_hz: u64) -> Self {
        let start_cycles = Self::get_ticks();

        Self {
            start_cycles,
            start_time: 0,
            tick_hz,
        }
    }

    /// Read hardware ticks with serialization fences on x86
    #[inline(always)]
    fn get_ticks() -> u64 {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        unsafe {
            // Load fence: prevents the CPU from executing rdtsc 
            // before previous instructions have finished.
            _mm_lfence();
            let res = _rdtsc();
            _mm_lfence();
            res
        }

        #[cfg(target_arch = "aarch64")]
        {
            let val: u64;
            unsafe {
                // ARM64 system counter is usually already synchronized
                std::arch::asm!("mrs {}, cntvct_el0", out(reg) val);
            }
            val
        }

        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
        0
    }

    /// Return elapsed time in **nanoseconds**.
    pub fn ns(&self) -> u128 {
        let end_ticks = Self::get_ticks();
        let delta = end_ticks.wrapping_sub(self.start_cycles);

        #[cfg(target_arch = "aarch64")]
        {
            // Apple Silicon hardware counter frequency is fixed at 24MHz
            // 1 / 24MHz = 41.666ns per tick.
            return (delta *1_000_000_000 / 24_000_000) as u128;
        }
        
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            // On Linux/x86, we use the tick_ghz passed at start
            return (delta*1_000_000_000 / self.tick_hz) as u128;
        }

        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
        {
            self.start_time.elapsed().as_nanos()
        }
    }
    
    pub fn us(&self) -> f64 { self.ns() as f64 / 1_000.0 }
    pub fn ms(&self) -> f64 { self.ns() as f64 / 1_000_000.0 }
}

