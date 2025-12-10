	# 为每个核心的栈定义大小，这里是 16KB
	.equ	STACK_SIZE, 1024*16

	# 将 _start 符号声明为全局可见，作为程序入口
	.global	_start

	# .text
	.section .text.entry
_start:
	# park harts with id != 0
	# 让非 0 号核心“待命”

	csrr	t0, mhartid		# 读 CSR 寄存器 mhartid，获取当前核心的ID，存入 t0 寄存器
	mv	tp, t0			# 将核心ID从 t0 移到 tp 寄存器中备份，以便后续使用
	bnez	t0, park		# bnez (Branch if Not Equal to Zero): 如果 t0 不为零（即核心ID不是0），则跳转到 park 标签

	# Setup stacks, the stack grows from bottom to top, so we put the
	# stack pointer to the very end of the stack range.
	# (注释有误，RISC-V 栈通常是向下生长的，但这里的计算逻辑是正确的)
	# 为 0 号核心设置栈

	slli	t0, t0, 14		# slli (Shift Left Logical Immediate): 将 t0 (此时必为0) 左移 14 位。
					# 1^14 = 16384，所以这行相当于 t0 = t0 * STACK_SIZE。对于0号核心，t0 仍然是 0。
	la	sp, stacks + STACK_SIZE	# la (Load Address): 将 `stacks` 标签的地址加上一个栈的大小(STACK_SIZE)，结果加载到 sp (栈指针)。
					# 这将 sp 指向了第一个栈空间的末尾（高地址处）。
	add	sp, sp, t0		# 将 sp 和 t0 相加。对于0号核心，t0是0，所以 sp 不变。
					# (对于其他核心，这行代码会将 sp 指向属于它们自己的栈空间)

	j rust_main
park:
	wfi				# wfi (Wait For Interrupt): 让CPU核心进入低功耗睡眠状态，直到被中断唤醒。
	j	park			# 非 0 号核心在这里无限循环等待，进入“待命”状态 😴

	# In the standard RISC-V calling convention, the stack pointer sp
	# is always 16-byte aligned.
	# 确保下面的 `stacks` 标签地址是 16 字节对齐的，这是一个好习惯，能提升性能。
.balign 16
stacks:
	# .skip 指令：在这里预留一块内存空间
	# STACK_SIZE * 1: 预留 1 个核心的栈空间
	# 这块内存就是所有核心的栈的总和。
	.skip	STACK_SIZE * 1

	.end				# 文件结束
