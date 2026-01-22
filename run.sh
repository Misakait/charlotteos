#!/bin/sh
KERNEL="target/riscv64gc-unknown-none-elf/release/charlotte_os"
KERNEL_BIN="target/riscv64gc-unknown-none-elf/release/charlotte_os.bin"
BIOS="rustsbi.bin"
cargo build --release

if [ $? -ne 0 ]; then
    echo "编译失败，调试会话中止。"
    exit 1
fi

rust-objcopy --binary-architecture=riscv64 $KERNEL --strip-all -O binary $KERNEL_BIN

qemu-system-riscv64 \
    -machine virt \
    -nographic \
    -smp 1 \
    -bios $BIOS \
    -kernel $KERNEL
