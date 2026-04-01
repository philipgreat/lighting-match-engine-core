#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

#[path = "kernel/mod.rs"]
mod kernel_support;

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use kernel_support::clock::{KernelClock, KernelStopwatch};
use kernel_support::engine_clock::{Clock, Stopwatch};
use kernel_support::match_demo::{DemoOrder, DemoOrderBook, Side};
use kernel_support::memory::init_heap;
use kernel_support::serial::serial;
use kernel_support::vga::writer;

const MULTIBOOT2_MAGIC: u32 = 0xe85250d6;
const MULTIBOOT2_ARCH: u32 = 0;
const MULTIBOOT2_LENGTH: u32 = 24;
const MULTIBOOT2_CHECKSUM: u32 =
    (0u32).wrapping_sub(MULTIBOOT2_MAGIC + MULTIBOOT2_ARCH + MULTIBOOT2_LENGTH);

#[unsafe(link_section = ".multiboot")]
#[unsafe(no_mangle)]
static MULTIBOOT2_HEADER: [u32; 6] = [
    MULTIBOOT2_MAGIC,
    MULTIBOOT2_ARCH,
    MULTIBOOT2_LENGTH,
    MULTIBOOT2_CHECKSUM,
    0,
    8,
];

global_asm!(
    r#"
    .section .text.boot, "ax"
    .global _start
_start:
    cli
    lea rsp, [rip + stack_top]
    xor rbp, rbp
    call kernel_main

1:
    hlt
    jmp 1b

    .section .bss.stack, "aw", @nobits
    .align 16
stack_bottom:
    .skip 16384
stack_top:
"#
);

#[unsafe(no_mangle)]
extern "C" fn kernel_main() -> ! {
    init_heap();

    let boot_clock = KernelClock::new();
    let startup_timer = KernelStopwatch::start();
    serial().init();
    let writer = writer();
    writer.clear();
    write_line_all("lighting-match-engine-core");
    write_line_all("booted under GRUB on bare metal");
    write_u64_all("clock_now_ns", boot_clock.now_ns());
    write_line_all("");

    let mut order_book = DemoOrderBook::new();
    order_book.seed(
        DemoOrder::new(1, Side::Sell, 101, 3),
        DemoOrder::new(2, Side::Sell, 102, 2),
    );
    let execution = order_book.match_order(DemoOrder::new(3, Side::Buy, 101, 4));

    write_line_all("match-engine smoke test:");
    write_line_all("incoming buy id=3 price=101 qty=4");
    match execution {
        Some(trade) => {
            write_line_all("matched against resting ask");
            write_u64_all("buy_order_id", trade.buy_order_id);
            write_u64_all("sell_order_id", trade.sell_order_id);
            write_u64_all("trade_price", trade.price);
            write_u64_all("trade_qty", trade.quantity as u64);
        }
        None => write_line_all("no trade produced"),
    }

    write_line_all("");
    write_u64_all("startup_elapsed_ns", startup_timer.elapsed_ns());
    write_line_all("kernel idle");

    halt_loop()
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    write_line_all("");
    write_line_all("kernel panic");
    write_fmt_line_all(format_args!("{}", info.message()));
    halt_loop()
}

#[alloc_error_handler]
fn alloc_error(_layout: core::alloc::Layout) -> ! {
    write_line_all("heap exhausted");
    halt_loop()
}

fn write_line_all(text: &str) {
    writer().write_line(text);
    serial().write_line(text);
}

fn write_u64_all(label: &str, value: u64) {
    writer().write_u64(label, value);
    serial().write_u64(label, value);
}

fn write_fmt_line_all(args: core::fmt::Arguments<'_>) {
    writer().write_fmt_line(args);
    serial().write_fmt_line(args);
}

fn halt_loop() -> ! {
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
