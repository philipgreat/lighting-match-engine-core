ENTRY(_start)

SECTIONS
{
  . = 1M;

  .multiboot ALIGN(8) : {
    KEEP(*(.multiboot))
  }

  .text ALIGN(4K) : {
    *(.text .text.*)
  }

  .rodata ALIGN(4K) : {
    *(.rodata .rodata.*)
  }

  .data ALIGN(4K) : {
    *(.data .data.*)
  }

  .bss ALIGN(4K) : {
    *(.bss .bss.*)
    *(COMMON)
  }
}
