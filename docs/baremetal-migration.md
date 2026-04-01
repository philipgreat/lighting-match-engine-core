# Bare-Metal / GRUB Migration Notes

This document explains what was changed to make this repository produce a bare-metal program that behaves like a small kernel and can be loaded by GRUB.

It also explains what has not been migrated yet, because the original project is still primarily a user-space Rust program.

## Goal

The goal of this change was not to fully convert the entire matching engine into a production-ready kernel in one step.

The goal was to:

1. Add a real `no_std` bare-metal kernel target.
2. Make that kernel recognizable by GRUB through a multiboot2 header.
3. Boot into Rust code without relying on a host operating system.
4. Run a small in-kernel matching demo to prove the project can now execute matching-related logic on bare metal.

## What Changed

### 1. Added a separate kernel binary target

New file:

- `src/bin/kernel.rs`

This file is the new bare-metal entry point.

Key changes in this file:

- Uses `#![no_std]` and `#![no_main]`.
- Defines a multiboot2 header so GRUB can load it.
- Defines `_start` through inline assembly.
- Sets up a bootstrap stack.
- Calls `kernel_main()` directly instead of relying on `main()` from `std`.
- Installs:
  - `#[panic_handler]`
  - `#[alloc_error_handler]`

This leaves the original `src/main.rs` intact as the user-space benchmark/program entry.

### 2. Added minimal kernel runtime support

New files:

- `src/bin/kernel/memory.rs`
- `src/bin/kernel/vga.rs`
- `src/bin/kernel/match_demo.rs`
- `src/bin/kernel/mod.rs`

#### `memory.rs`

Implements a minimal bump allocator so the kernel can use `alloc` containers.

Current behavior:

- Fixed-size heap.
- Monotonic allocation only.
- No free list.
- Good enough for early boot and simple demos.

#### `vga.rs`

Implements direct VGA text buffer output.

Current behavior:

- Writes directly to physical address `0xb8000`.
- Supports line output.
- Supports simple decimal number printing.
- Supports scrolling within 80x25 text mode.

This is how the kernel reports boot status without `println!`.

#### `match_demo.rs`

Implements a small, self-contained order matching demo that works under `no_std + alloc`.

Purpose:

- Demonstrate that matching-related Rust logic can execute in the bare-metal environment.
- Avoid pulling the full existing engine into the kernel before refactoring its dependencies.

### 3. Added linker and GRUB configuration

New files:

- `linker.ld`
- `grub/grub.cfg`

#### `linker.ld`

Defines:

- Kernel entry point: `_start`
- Load address: `1M`
- Section layout for:
  - `.multiboot`
  - `.text`
  - `.rodata`
  - `.data`
  - `.bss`

This is required so the kernel image is linked in a layout suitable for boot loading.

#### `grub.cfg`

Defines a GRUB menu entry that loads:

- `/boot/kernel.elf`

using:

- `multiboot2`

### 4. Added build scripts for the kernel workflow

New files:

- `scripts/build-kernel.sh`
- `scripts/build-grub-iso.sh`

#### `build-kernel.sh`

Builds the kernel with:

- nightly Rust
- `-Z build-std=core,alloc`
- target `x86_64-unknown-none`
- custom linker script via `-T linker.ld`

#### `build-grub-iso.sh`

Builds the kernel and assembles the expected GRUB ISO tree:

- `boot/kernel.elf`
- `boot/grub/grub.cfg`

If `grub-mkrescue` is installed, this script can also create a bootable ISO image.

### 5. Adjusted Cargo configuration

Changed file:

- `Cargo.toml`

Changes made:

- Set `panic = "abort"` for `dev` and `release`.
- Moved `ahash` and `libc` behind:
  - `cfg(not(target_os = "none"))`

Reason:

The original crate dependencies are valid for host OS builds, but not for a `target_os = "none"` kernel build.

Without this split, Cargo tries to compile unsupported host-oriented crates for bare metal.

### 6. Added local Cargo config

New file:

- `.cargo/config.toml`

This currently only standardizes the target directory.

### 7. Updated README

Changed file:

- `README.md`

The README now includes:

- how to build the kernel ELF
- how to try building the GRUB ISO
- where the outputs are written

## What Was Not Migrated Yet

The original matching engine has not been fully moved into the kernel.

This is intentional.

The existing codebase still depends heavily on user-space facilities, including:

- `std`
- environment arguments
- file output
- wall-clock/system time APIs
- libc-backed OS features
- platform affinity helpers

Examples in the current repo include:

- command-line config parsing
- performance file export
- OS timers
- CPU affinity
- host-side formatted output

Those pieces do not transfer directly into a bare-metal kernel.

## Current Bare-Metal Capability

After this change, the project can do the following:

1. Build a `no_std` x86_64 kernel ELF.
2. Expose a valid multiboot2 header.
3. Be loaded by GRUB.
4. Enter Rust code on bare metal.
5. Print directly to VGA memory.
6. Allocate from a tiny internal heap.
7. Execute a small matching demo inside the kernel.

## Current Limitations

This is still an early-stage kernel skeleton, not a full operating system or full production trading kernel.

Current limitations include:

- No interrupt descriptor table.
- No GDT setup beyond what the bootloader provides.
- No paging or virtual memory management of our own.
- No serial console.
- No timer interrupt support.
- No keyboard or storage drivers.
- No networking.
- No task scheduler.
- No SMP bring-up.
- No integration of the original `DenseOrderBook` and surrounding engine modules.

Also, VGA text output assumes booting in an environment where text mode memory at `0xb8000` is valid.

## Why the Full Engine Was Not Moved Immediately

The current engine code is not organized yet as a portable core plus host adapters.

Right now, it mixes:

- pure matching logic
- benchmarking logic
- CLI/config logic
- timing logic
- file output
- platform-specific helpers

A proper bare-metal migration should first separate the engine into layers.

Recommended split:

1. Pure engine core:
   - `no_std` or `alloc` friendly
   - no OS calls
   - no file or env dependencies
2. Host runtime:
   - current benchmark executable
   - CLI parsing
   - file export
   - timing and diagnostics
3. Kernel runtime:
   - boot entry
   - allocator
   - output device
   - later, interrupts/timers/drivers

## Recommended Next Steps

To continue the migration cleanly, the next steps should be:

### Step 1: Extract a portable engine core

Move the real matching data structures and algorithms into a module or crate that depends on:

- `core`
- `alloc`

but not on:

- `std`
- `libc`
- environment variables
- files

### Step 2: Replace user-space-only utilities behind traits

Abstract these concerns:

- time source
- logging/output
- persistence/export

Example direction:

- `Clock` trait
- `Output` trait
- optional feature-gated diagnostics

### Step 3: Port `DenseOrderBook` first

The best next migration target is likely:

- `data_types.rs`
- `dense_order_book.rs`

because that is the real core matching path.

This will likely require replacing or isolating:

- `ahash`
- `std::collections`
- timer coupling

### Step 4: Add serial output

For real debugging under QEMU or hardware, serial output is much more practical than VGA alone.

Recommended next addition:

- COM1 serial writer
- QEMU debug flow such as `-serial stdio`

### Step 5: Add a boot test path

Once GRUB tools and a VM path are available, add a test script that:

1. builds the kernel
2. builds the ISO
3. boots it under QEMU
4. checks expected output

## Build Commands

Build the kernel ELF:

```bash
bash scripts/build-kernel.sh
```

Expected output:

```bash
target/x86_64-unknown-none/release/kernel
```

Try to build a GRUB ISO:

```bash
bash scripts/build-grub-iso.sh
```

Expected ISO output if GRUB tools are installed:

```bash
target/lighting-match-engine-core.iso
```

## Environment Limitation Observed During This Work

The current machine successfully builds the kernel ELF, but does not currently provide `grub-mkrescue` in `PATH`.

That means:

- kernel build works
- GRUB directory assembly works
- final ISO generation is pending installation of GRUB tooling

## Summary

This change converts the repository from "user-space only" into "user-space program plus bootable bare-metal kernel target".

The most important outcome is that the repo now has a real kernel entry path and build flow.

The most important remaining work is to refactor the actual matching engine into a portable `alloc`-based core so it can replace the temporary in-kernel demo.
