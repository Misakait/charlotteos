# .gdbinit
source ~/.gdbinit.1
# 加载内核文件以获取符号信息
file target/riscv64gc-unknown-none-elf/release/charlotte_os

# 设置目标架构
set architecture riscv:rv64

# 连接到 QEMU 在 1234 端口上提供的调试服务
target remote :1234

# (可选) 设置初始断点
b rust_main
