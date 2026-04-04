#!/bin/sh

KERNEL="target/riscv64gc-unknown-none-elf/release/charlotte_os"
BIOS="rustsbi.bin"

cargo build --release
if [ $? -ne 0 ]; then
    echo "编译失败，调试会话中止。"
    exit 1
fi

# 清理之前可能意外残留的 QEMU 进程
if [ -f .qemu.pid ]; then
    kill -9 $(cat .qemu.pid) 2>/dev/null
    rm .qemu.pid
fi

qemu-system-riscv64 \
    -machine virt \
    -nographic \
    -smp 1 \
    -bios $BIOS \
    -kernel $KERNEL \
    -S \
    -s

# 保存进程 PID
echo $! > .qemu.pid
