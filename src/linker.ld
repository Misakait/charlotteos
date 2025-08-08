/* linker.ld */

/* 1. 指定输出文件的架构和入口点 */
OUTPUT_ARCH(riscv)
ENTRY(_start)

/* 2. 定义内存的基地址 */
/* 对于纯M模式内核，我们是第一个程序，所以可以直接使用QEMU virt机器的DRAM起始地址 */
BASE_ADDRESS = 0x80000000;

/* 3. 定义内存区域以及属性 */
MEMORY
{
    ram (rwxa) : ORIGIN = 0x80000000, LENGTH = 128M
	/*ram   (wxa!ri) : ORIGIN = 0x80000000, LENGTH = 128M*/
}

/* 4. 定义段(SECTIONS)，这是核心部分 */
SECTIONS
{
    /* Location Counter (.), 表示当前地址. 我们将它设置为基地址 */
    . = BASE_ADDRESS;

    /* .text 段: 存放所有可执行代码 */
    /* ALIGN(4) 确保代码段4字节对齐 */
    .text : ALIGN(4) {
        /* 定义一个名为 _text_start 的符号，其地址为当前位置 */
        PROVIDE(_text_start = .);

        /* 将所有输入文件(.o)中的 .text.entry 段放在最前面 */
        /* 我们在 entry.S 中定义的 _start 就在这里 */
        *(.text.entry)

        /* 接着放入所有其他文件的 .text 段 */
        *(.text .text.*)

        PROVIDE(_text_end = .);
    } > ram

    /* .rodata 段: 存放所有只读数据, 例如字符串字面量 */
    .rodata : ALIGN(4) {
        PROVIDE(_rodata_start = .);
        *(.rodata .rodata.*)
        PROVIDE(_rodata_end = .);
    } > ram

    /* .data 段: 存放所有已初始化的全局变量和静态变量 */
    .data : ALIGN(4) {
        PROVIDE(_data_start = .);
        *(.data .data.*)
        PROVIDE(_data_end = .);
    } > ram

    /* .bss 段: 存放所有未初始化的全局变量和静态变量 */
    /* 内核启动后，需要自己负责将这块内存清零 */
    .bss : ALIGN(4) {
        PROVIDE(_bss_start = .);
        *(.sbss .sbss.*)
        *(.bss .bss.*)
        *(COMMON)
        PROVIDE(_bss_end = .);
    } > ram


    PROVIDE(_memory_start = ORIGIN(ram));
    PROVIDE(_memory_end = ORIGIN(ram) + LENGTH(ram));

    /* .heap 段: 用于动态分配内存 */
    PROVIDE(_heap_start = _bss_end);
    PROVIDE(_heap_size = _memory_end - _heap_start);
}
