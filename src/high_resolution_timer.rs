#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use core::arch::x86_64::{_rdtsc, _mm_lfence};

#[cfg(windows)]
use windows_sys::Win32::System::Performance::{
    QueryPerformanceCounter,
    QueryPerformanceFrequency,
};

use std::sync::OnceLock;

/// ------------------------------------------------------------
/// High-Resolution Timer (Cross-Platform)
/// ------------------------------------------------------------
/// • Windows: QueryPerformanceCounter
/// • x86_64 (Linux/macOS): rdtsc + startup calibration
/// • ARM64 (Linux/macOS): cntvct_el0 + cntfrq_el0
/// ------------------------------------------------------------
#[derive(Debug)]
pub struct HighResolutionTimer {
    start_cycles: u64
}

// ==========================
// Global tick frequency (Hz)
// ==========================

static TICK_HZ: OnceLock<u64> = OnceLock::new();

#[inline(always)]
fn global_tick_hz() -> u64 {
    *TICK_HZ.get_or_init(|| calibrate_tick_hz())
}

impl HighResolutionTimer {
    /// Start the timer.
    ///
    /// 
    /// - tick_hz not belongs to instance
    /// - calibrate on start
    pub fn start() -> Self {
        // 触发一次校准（如果还没做）
        let _ = global_tick_hz();

        let start_cycles = Self::get_ticks();

        Self {
            start_cycles
        }
    }

    /// Read hardware ticks
    #[inline(always)]
    fn get_ticks() -> u64 {
        // --------------------------
        // Windows
        // --------------------------
        #[cfg(windows)]
        unsafe {
            let mut v: i64 = 0;
            QueryPerformanceCounter(&mut v);
            return v as u64;
        }

        // --------------------------
        // x86 (Linux / macOS)
        // --------------------------
        #[cfg(all(
            not(windows),
            any(target_arch = "x86", target_arch = "x86_64")
        ))]
        unsafe {
            _mm_lfence();
            let t = _rdtsc();
            _mm_lfence();
            return t;
        }

        // --------------------------
        // ARM64 (Linux / macOS)
        // --------------------------
        #[cfg(target_arch = "aarch64")]
        {
            let val: u64;
            unsafe {
                core::arch::asm!("mrs {}, cntvct_el0", out(reg) val);
            }
            return val;
        }

        // --------------------------
        // Fallback
        // --------------------------
        #[cfg(not(any(
            windows,
            target_arch = "x86",
            target_arch = "x86_64",
            target_arch = "aarch64"
        )))]
        {
            0
        }
    }

    /// Return elapsed time in **nanoseconds** (integer)
    pub fn ns(&self) -> u128 {
        let end_ticks = Self::get_ticks();
        let delta = end_ticks.wrapping_sub(self.start_cycles) as u128;

        (delta * 1_000_000_000u128) / global_tick_hz() as u128
    }

    pub fn us(&self) -> u64 {
        (self.ns() / 1_000) as u64
    }

    pub fn ms(&self) -> u64 {
        (self.ns() / 1_000_000) as u64
    }
}

// ============================================================
// Tick calibration
// ============================================================

fn calibrate_tick_hz() -> u64 {
    // --------------------------
    // Windows: QPC frequency
    // --------------------------
    #[cfg(windows)]
    unsafe {
        let mut freq: i64 = 0;
        QueryPerformanceFrequency(&mut freq);
        return freq as u64;
    }

    // --------------------------
    // x86 Linux / macOS
    // --------------------------
    #[cfg(all(
        not(windows),
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    {
        return calibrate_tsc_with_monotonic();
    }

    // --------------------------
    // ARM64
    // --------------------------
    #[cfg(target_arch = "aarch64")]
    {
        return read_cntfrq_el0();
    }
    // fallback
    2_500_000_000
}

// --------------------------
// x86 TSC calibration (integer)
// --------------------------

#[cfg(all(
    not(windows),
    any(target_arch = "x86", target_arch = "x86_64")
))]
fn calibrate_tsc_with_monotonic() -> u64 {
    use libc::{clock_gettime, timespec, CLOCK_MONOTONIC_RAW};

    unsafe {
        let mut ts_start = timespec { tv_sec: 0, tv_nsec: 0 };
        let mut ts_end = timespec { tv_sec: 0, tv_nsec: 0 };

        clock_gettime(CLOCK_MONOTONIC_RAW, &mut ts_start);
        _mm_lfence();
        let tsc_start = _rdtsc();
        _mm_lfence();

        spin_wait_ns(10_000_000); // ~10ms

        clock_gettime(CLOCK_MONOTONIC_RAW, &mut ts_end);
        _mm_lfence();
        let tsc_end = _rdtsc();
        _mm_lfence();

        let delta_tsc = tsc_end - tsc_start;
        let delta_ns =
            (ts_end.tv_sec - ts_start.tv_sec) as u128 * 1_000_000_000u128 +
            (ts_end.tv_nsec - ts_start.tv_nsec) as u128;

        (delta_tsc as u128 * 1_000_000_000u128 / delta_ns) as u64
    }
}

#[cfg(all(
    not(windows),
    any(target_arch = "x86", target_arch = "x86_64")
))]
#[inline(always)]
fn spin_wait_ns(ns: u64) {
    use libc::{clock_gettime, timespec, CLOCK_MONOTONIC_RAW};

    let start = unsafe {
        let mut ts = timespec { tv_sec: 0, tv_nsec: 0 };
        clock_gettime(CLOCK_MONOTONIC_RAW, &mut ts);
        ts.tv_sec as u128 * 1_000_000_000u128 + ts.tv_nsec as u128
    };

    loop {
        let now = unsafe {
            let mut ts = timespec { tv_sec: 0, tv_nsec: 0 };
            clock_gettime(CLOCK_MONOTONIC_RAW, &mut ts);
            ts.tv_sec as u128 * 1_000_000_000u128 + ts.tv_nsec as u128
        };
        if now - start >= ns as u128 {
            break;
        }
    }
}

// --------------------------
// ARM64 frequency
// --------------------------

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn read_cntfrq_el0() -> u64 {
    let freq: u64;
    unsafe {
        core::arch::asm!("mrs {}, cntfrq_el0", out(reg) freq);
    }
    freq
}
