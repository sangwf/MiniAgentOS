.section .text._start, "ax"
.global _start
.type _start, %function

_start:
    mov x19, x0
    ldr x0, =__stack_top
    mov sp, x0
    // Install exception vector table.
    ldr x0, =__vectors_start
    msr vbar_el1, x0
    isb

    // Disable caches (C and I) for DMA simplicity in early bring-up.
    mrs x0, sctlr_el1
    bic x0, x0, #(1 << 2)    // C: data cache
    bic x0, x0, #(1 << 12)   // I: instruction cache
    msr sctlr_el1, x0
    isb
    // Enable FP/ASIMD at EL1 to avoid traps from core alignment helpers.
    mrs x0, cpacr_el1
    orr x0, x0, #(3 << 20)
    msr cpacr_el1, x0
    isb

    // Zero .bss
    ldr x0, =__bss_start
    ldr x1, =__bss_end
1:
    cmp x0, x1
    b.ge 2f
    str xzr, [x0], #8
    b 1b
2:
    mov x0, x19
    bl kmain

3:
    wfe
    b 3b

    .align 11
__vectors_start:
    // Current EL with SP0
    mov x0, #0
    b exception_entry
    .balign 0x80
    mov x0, #1
    b exception_entry
    .balign 0x80
    mov x0, #2
    b exception_entry
    .balign 0x80
    mov x0, #3
    b exception_entry
    .balign 0x80
    // Current EL with SPx
    mov x0, #4
    b exception_entry
    .balign 0x80
    mov x0, #5
    b exception_entry
    .balign 0x80
    mov x0, #6
    b exception_entry
    .balign 0x80
    mov x0, #7
    b exception_entry
    .balign 0x80
    // Lower EL using AArch64
    mov x0, #8
    b exception_entry
    .balign 0x80
    mov x0, #9
    b exception_entry
    .balign 0x80
    mov x0, #10
    b exception_entry
    .balign 0x80
    mov x0, #11
    b exception_entry
    .balign 0x80
    // Lower EL using AArch32
    mov x0, #12
    b exception_entry
    .balign 0x80
    mov x0, #13
    b exception_entry
    .balign 0x80
    mov x0, #14
    b exception_entry
    .balign 0x80
    mov x0, #15
    b exception_entry
