#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use core::arch::x86_64::_rdtsc;
use std::time::{Duration, Instant};

/// ------------------------------------------------------------
/// High-Resolution Timer (Cross-Platform)
/// ------------------------------------------------------------
/// • Uses CPU hardware timestamp counter (TSC) on x86/x86_64
/// • Falls back to `Instant::now()` on non-x86 platforms (e.g. Apple Silicon)
/// • Nanosecond precision (depending on hardware & OS)
/// • Lightweight, lock-free, no allocations
///
/// Example:
/// ```
/// let timer = HighResCounter::start(5.0); // CPU at 5.0 GHz
/// do_work();
/// println!("Elapsed: {} ns", timer.ns());
/// ```
pub struct HighResultionCounter {
    start_cycles: u64,
    start_time: Instant,
    cpu_ghz: f64,
}

impl HighResultionCounter {
    /// Start the high-resolution timer.
    ///
    /// `cpu_ghz` – your CPU base frequency in GHz
    /// (e.g. 3.5 for 3.5 GHz, 5.0 for 5 GHz)
    pub fn start(cpu_ghz: f64) -> Self {
        // Capture the starting CPU cycle counter if supported
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        let start_cycles = unsafe { _rdtsc() };

        // Fallback for non-x86 (e.g. ARM / Apple Silicon)
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        let start_cycles = 0;

        Self {
            start_cycles,
            start_time: Instant::now(),
            cpu_ghz,
        }
    }

    /// Return elapsed time in **nanoseconds**.
    pub fn ns(&self) -> u128 {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            let end = unsafe { _rdtsc() };
            let delta_cycles = end - self.start_cycles;
            // Convert cycles → nanoseconds
            let ns = (delta_cycles as f64 / self.cpu_ghz) as u128;
            return ns;
        }

        // Fallback using `Instant::elapsed`
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            return self.start_time.elapsed().as_nanos();
        }
    }

    /// Return elapsed time in **microseconds** (float).
    pub fn us(&self) -> f64 {
        self.ns() as f64 / 1_000.0
    }

    /// Return elapsed time in **milliseconds** (float).
    pub fn ms(&self) -> f64 {
        self.ns() as f64 / 1_000_000.0
    }

    /// Return elapsed time as a standard `Duration`.
    pub fn duration(&self) -> Duration {
        Duration::from_nanos(self.ns() as u64)
    }
}

// fn main() {
//     // You can obtain the CPU frequency via:
//     //  • Linux:   `lscpu | grep "MHz"`
//     //  • macOS:   `sysctl hw.cpufrequency`
//     //  • Windows: PowerShell → `(Get-CimInstance Win32_Processor).MaxClockSpeed`
//     let timer = HighResuCounter::start(5.0); // 5 GHz CPU

//     // --- Code to measure ---
//     let mut sum = 0u64;
//     for i in 0..1_000_000 {
//         sum = sum.wrapping_add(i);
//     }

//     println!("Elapsed: {} ns", timer.ns());
// }
