/* linker.ld */

/* 1. 指定输出文件的架构和入口点 */
OUTPUT_ARCH(riscv)
ENTRY(_start)

/* 2. 定义内存的基地址 */
/* 对于S模式内核，RustSBI 后内核从 0x80200000 开始 */
PHYS_BASE = 0x80200000;
/* 更新为内核运行在内存高半区 */
BASE_ADDRESS = 0xffffffff80200000;
OFFSET = BASE_ADDRESS - PHYS_BASE;

/* 3. 定义内存区域以及属性 */

/*MEMORY
{
    ram (rwxa) : ORIGIN = 0x80200000, LENGTH = 126M
	/*ram   (wxa!ri) : ORIGIN = 0x80000000, LENGTH = 128M*/
/*}*/

/* 4. 定义段(SECTIONS)，这是核心部分 */
SECTIONS
{
    /* Location Counter (.), 表示当前地址. 我们将它设置为基地址 */
    . = BASE_ADDRESS;
    PROVIDE(_skernel = .);
    /* .text 段: 存放所有可执行代码 */
    /* ALIGN(4K) 确保代码段4K字节对齐 */
    .text : AT(ADDR(.text) - OFFSET) ALIGN(4K) {
        /* 定义一个名为 _text_start 的符号，其地址为当前位置 */
        PROVIDE(_text_start = .);

        /* 将所有输入文件(.o)中的 .text.entry 段放在最前面 */
        /* 我们在 entry.S 中定义的 _start 就在这里 */
        *(.text.entry)

        /* 接着放入所有其他文件的 .text 段 */
        *(.text .text.*)

        PROVIDE(_text_end = .);
    } /* > ram */

    /* .rodata 段: 存放所有只读数据, 例如字符串字面量 */
    .rodata : AT(ADDR(.rodata) - OFFSET) ALIGN(4K) {
        PROVIDE(_rodata_start = .);
        *(.rodata .rodata.*)
        PROVIDE(_rodata_end = .);
    } /* > ram */

    /* .data 段: 存放所有已初始化的全局变量和静态变量 */
    .data : AT(ADDR(.data) - OFFSET) ALIGN(4K) {
        PROVIDE(_data_start = .);
        *(.data .data.*)
        PROVIDE(_data_end = .);
    } /* > ram */

    /* .bss 段: 存放所有未初始化的全局变量和静态变量 */
    /* 内核启动后，需要自己负责将这块内存清零 */
    .bss : AT(ADDR(.bss) - OFFSET) ALIGN(4K) {
        PROVIDE(_bss_start_with_stack = .);
        *(.bss.stack)
        PROVIDE(_bss_start = .);
        *(.sbss .sbss.*)
        *(.bss .bss.*)
        *(COMMON)
        PROVIDE(_bss_end = .);
    } /* > ram */

    PROVIDE(_ekernel = _bss_end);

   /* PROVIDE(_memory_start = ORIGIN(ram));
    PROVIDE(_memory_end = ORIGIN(ram) + LENGTH(ram));

    PROVIDE(_ekernel = _bss_end);
    PROVIDE(_heap_size = _memory_end - _ekernel); */
}
