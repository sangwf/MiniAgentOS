ENTRY(_start)

SECTIONS
{
  . = 0x40080000;

  .text : {
    KEEP(*(.text._start))
    *(.text .text.*)
  }

  .rodata : {
    *(.rodata .rodata.*)
  }

  .data : {
    *(.data .data.*)
  }

  .bss (NOLOAD) : {
    __bss_start = .;
    *(.bss .bss.* COMMON)
    __bss_end = .;
  }

  .stack (NOLOAD) : {
    . = ALIGN(16);
    __stack_bottom = .;
    . = . + 0x40000; /* 256 KiB: mbedTLS handshake/debug paths exceed the old 16 KiB stack */
    __stack_top = .;
  }
}
