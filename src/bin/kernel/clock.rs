use super::engine_clock::{Clock, Stopwatch};

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::__cpuid;

#[cfg(target_arch = "x86_64")]
use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default, Clone, Copy)]
pub struct KernelClock;

#[derive(Debug, Clone, Copy)]
pub struct KernelStopwatch {
    start_ticks: u64,
}

impl KernelClock {
    pub const fn new() -> Self {
        Self
    }
}

impl Clock for KernelClock {
    fn now_ns(&self) -> u64 {
        ticks_to_ns(read_ticks())
    }
}

impl KernelStopwatch {
    pub fn start() -> Self {
        Self {
            start_ticks: read_ticks(),
        }
    }
}

impl Stopwatch for KernelStopwatch {
    fn elapsed_ns(&self) -> u64 {
        ticks_to_ns(read_ticks().wrapping_sub(self.start_ticks))
    }
}

#[cfg(target_arch = "x86_64")]
fn read_ticks() -> u64 {
    let ticks: u64;
    unsafe {
        core::arch::asm!(
            "lfence",
            "rdtsc",
            "shl rdx, 32",
            "or rax, rdx",
            out("rax") ticks,
            out("rdx") _,
            options(nostack, preserves_flags)
        );
    }
    ticks
}

#[cfg(target_arch = "aarch64")]
fn read_ticks() -> u64 {
    let ticks: u64;
    unsafe {
        core::arch::asm!("mrs {}, cntvct_el0", out(reg) ticks, options(nostack, preserves_flags));
    }
    ticks
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn read_ticks() -> u64 {
    0
}

#[cfg(target_arch = "x86_64")]
fn ticks_to_ns(ticks: u64) -> u64 {
    let hz = tsc_hz();
    ((ticks as u128 * 1_000_000_000u128) / hz as u128) as u64
}

#[cfg(target_arch = "aarch64")]
fn ticks_to_ns(ticks: u64) -> u64 {
    let freq: u64;
    unsafe {
        core::arch::asm!("mrs {}, cntfrq_el0", out(reg) freq, options(nostack, preserves_flags));
    }
    ((ticks as u128 * 1_000_000_000u128) / freq as u128) as u64
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn ticks_to_ns(ticks: u64) -> u64 {
    ticks
}

#[cfg(target_arch = "x86_64")]
fn tsc_hz() -> u64 {
    static TSC_HZ: AtomicU64 = AtomicU64::new(0);

    let cached = TSC_HZ.load(Ordering::Relaxed);
    if cached != 0 {
        return cached;
    }

    let detected = detect_tsc_hz().unwrap_or(1_000_000_000);
    TSC_HZ.store(detected, Ordering::Relaxed);
    detected
}

#[cfg(target_arch = "x86_64")]
fn detect_tsc_hz() -> Option<u64> {
    let max_leaf = __cpuid(0).eax;

    if max_leaf >= 0x15 {
        let leaf_15 = __cpuid(0x15);
        if leaf_15.eax != 0 && leaf_15.ebx != 0 {
            let crystal_hz = if leaf_15.ecx != 0 {
                leaf_15.ecx as u64
            } else if max_leaf >= 0x16 {
                let leaf_16 = __cpuid(0x16);
                if leaf_16.eax != 0 {
                    (leaf_16.eax as u64) * 1_000_000
                } else {
                    0
                }
            } else {
                0
            };

            if crystal_hz != 0 {
                return Some(crystal_hz.saturating_mul(leaf_15.ebx as u64) / leaf_15.eax as u64);
            }
        }
    }

    if max_leaf >= 0x16 {
        let leaf_16 = __cpuid(0x16);
        if leaf_16.eax != 0 {
            return Some((leaf_16.eax as u64) * 1_000_000);
        }
    }

    None
}
