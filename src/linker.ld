/* linker.ld */

/* 1. 指定输出文件的架构和入口点 */
OUTPUT_ARCH(riscv)
ENTRY(_start)

/* 2. 定义内存的基地址 */
/* 对于纯M模式内核，我们是第一个程序，所以可以直接使用QEMU virt机器的DRAM起始地址 */
BASE_ADDRESS = 0x80000000;

/* 3. 定义段(SECTIONS)，这是核心部分 */
SECTIONS
{
    /* Location Counter (.), 表示当前地址. 我们将它设置为基地址 */
    . = BASE_ADDRESS;

    /* .text 段: 存放所有可执行代码 */
    /* ALIGN(4) 确保代码段4字节对齐 */
    .text : ALIGN(4) {
        /* 将所有输入文件(.o)中的 .text.entry 段放在最前面 */
        /* 我们在 entry.S 中定义的 _start 就在这里 */
        *(.text.entry)

        /* 接着放入所有其他文件的 .text 段 */
        *(.text .text.*)
    }

    /* .rodata 段: 存放所有只读数据, 例如字符串字面量 */
    .rodata : ALIGN(4) {
        *(.rodata .rodata.*)
    }

    /* .data 段: 存放所有已初始化的全局变量和静态变量 */
    .data : ALIGN(4) {
        *(.data .data.*)
    }

    /* .bss 段: 存放所有未初始化的全局变量和静态变量 */
    /* 内核启动后，需要自己负责将这块内存清零 */
    .bss : ALIGN(4) {
        *(.bss .bss.*)
    }

    /* 定义一个名为 end 的符号，其地址为当前位置 */
    /* 这个符号非常有用，它标记了内核镜像的结束位置。*/
    /* 我们在 entry.S 中用它来设置初始的栈顶指针！ */
    PROVIDE(end = .);
}
