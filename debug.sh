#!/bin/sh

# 定义内核文件路径 (请根据您的编译模式修改 debug 或 release)
KERNEL="target/riscv64gc-unknown-none-elf/release/charlotte_os"

# 1. 首先，编译项目以确保内核文件是最新的
#    我们传递脚本收到的所有参数 (例如 --release) 给 cargo build
cargo build --release

# 2. 检查编译是否成功
if [ $? -ne 0 ]; then
    echo "编译失败，调试会话中止。"
    exit 1
fi

# 3. 在后台启动 QEMU，并让它暂停等待 GDB
#    `&` 符号表示让这个命令在后台运行
qemu-system-riscv64 \
    -machine virt \
    -nographic \
    -smp 1 \
    -bios none \
    -kernel $KERNEL \
    -S \
    -s &

# 4. 保存 QEMU 进程的 ID (PID)
QEMU_PID=$!

# 5. 启动 GDB，它会自动读取 .gdbinit 文件并连接
#    使用您系统上的正确 GDB 命令
# gdb-multiarch
gdb
# 6. 当 GDB 退出后 (您在GDB中输入quit)，脚本会继续执行
#    杀死后台的 QEMU 进程，完成清理工作
echo "GDB会话结束，正在关闭QEMU..."
kill $QEMU_PID
