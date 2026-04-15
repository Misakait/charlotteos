# Charlotte OS

Charlotte OS is a personal `no_std` Rust operating system kernel for `riscv64` platforms. It is designed and tested with QEMU `virt` and RustSBI, and it currently implements early boot, memory management, task scheduling, traps/interrupt handling, a UART-based console, and a basic syscall layer.

This project is a personal operating system project, not a course experiment.

## Overview

This kernel boots directly from Rust and performs the following high-level initialization steps:

1. Enter the kernel from the boot entry assembly.
2. Enable an early MMU mapping so the kernel can switch into a higher-half virtual address space.
3. Parse the device tree blob to discover RAM and reserved regions.
4. Initialize memory management, including page tables and allocators.
5. Initialize the scheduler and spawn demo tasks.
6. Enable traps and interrupts.
7. Run the task scheduler.

The current codebase is centered around a small personal OS prototype. It includes example tasks and a shell-like task to exercise scheduling, I/O, and syscall paths.

## Features

- `no_std` / `no_main` Rust kernel
- RISC-V `Sv39` paging support
- Early boot MMU setup and higher-half mapping
- Memory block tracking and allocator initialization
- Buddy-based physical page allocation
- Page table management
- UART console output
- Interrupt and trap handling
- Timer-based task scheduling
- Task creation, blocking, waking, and exit flow
- Minimal syscall layer
- QEMU `virt` board support

## Project Structure

```text
charlotteos/
├── Cargo.toml
├── run.sh
├── debug.sh
├── debug.gdb
├── start_qemu.sh
├── rustsbi.bin
└── src/
    ├── main.rs
    ├── entry.S
    ├── bsp/
    ├── console/
    ├── data_struct/
    ├── driver/
    ├── mm/
    ├── task/
    ├── trap/
    ├── syslib/
    └── userlib/
```

## Requirements

To build and run the kernel, you need:

- Rust toolchain with support for the `riscv64gc-unknown-none-elf` target
- `cargo`
- `qemu-system-riscv64`
- `rust-gdb` or a compatible GDB for debugging
- The provided `rustsbi.bin` firmware image

If you do not already have the RISC-V target installed, add it with:

```text
rustup target add riscv64gc-unknown-none-elf
```

## Build

Build the kernel in release mode:

```text
cargo build --release
```

The output kernel is expected at:

```text
target/riscv64gc-unknown-none-elf/release/charlotte_os
```

## Run

The repository provides a helper script to build and launch the kernel in QEMU:

```text
./run.sh
```

This script:

- builds the project in release mode
- starts `qemu-system-riscv64`
- boots with `rustsbi.bin`
- loads the kernel image
- runs in `-nographic` mode

You can also run QEMU manually with settings similar to:

```text
qemu-system-riscv64 \
    -machine virt \
    -nographic \
    -smp 1 \
    -bios rustsbi.bin \
    -kernel target/riscv64gc-unknown-none-elf/release/charlotte_os
```

## Debug

Use the debug helper script to build the kernel and launch a GDB session:

```text
./debug.sh
```

Typical workflow:

1. Start the kernel under QEMU in a paused state.
2. Attach `rust-gdb`.
3. Load the provided GDB commands from `debug.gdb`.
4. Set breakpoints and step through early boot and scheduling code.

If you want to inspect the QEMU startup flow manually, see `debug.gdb` and `start_qemu.sh`.

## Notes

- The kernel currently uses a higher-half virtual memory layout with a fixed physical-to-virtual offset.
- Demo tasks are spawned during initialization to exercise scheduling and syscall behavior.
- Console output uses the kernel UART and SBI helper printing macros.
- The default Cargo feature enables UART interrupt support.
