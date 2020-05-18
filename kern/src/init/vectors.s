.global context_save
context_save:
    // Save x registers
    stp x26, x27, [sp, #-16]!
    stp x24, x25, [sp, #-16]!
    stp x22, x23, [sp, #-16]!
    stp x20, x21, [sp, #-16]!
    stp x18, x19, [sp, #-16]!
    stp x16, x17, [sp, #-16]!
    stp x14, x15, [sp, #-16]!
    stp x12, x13, [sp, #-16]!
    stp x10, x11, [sp, #-16]!
    stp x8, x9, [sp, #-16]!
    stp x6, x7, [sp, #-16]!
    stp x4, x5, [sp, #-16]!
    stp x2, x3, [sp, #-16]!
    stp x0, x1, [sp, #-16]!

    // Save q registers
    stp q30, q31, [sp, #-32]!
    stp q28, q29, [sp, #-32]!
    stp q26, q27, [sp, #-32]!
    stp q24, q25, [sp, #-32]!
    stp q22, q23, [sp, #-32]!
    stp q20, q21, [sp, #-32]!
    stp q18, q19, [sp, #-32]!
    stp q16, q17, [sp, #-32]!
    stp q14, q15, [sp, #-32]!
    stp q12, q13, [sp, #-32]!
    stp q10, q11, [sp, #-32]!
    stp q8, q9, [sp, #-32]!
    stp q6, q7, [sp, #-32]!
    stp q4, q5, [sp, #-32]!
    stp q2, q3, [sp, #-32]!
    stp q0, q1, [sp, #-32]!

    // Save special registers
    mrs x20, tpidr_el0
    mrs x21, sp_el0
    mrs x22, spsr_el1
    mrs x23, elr_el1
    mrs x24, ttbr0_el1
    mrs x25, ttbr1_el1

    stp x24, x25, [sp, #-16]!
    stp x22, x23, [sp, #-16]! 
    stp x20, x21, [sp, #-16]! 
    // ----TOP (neg)----
    // TPIDR
    // SP
    // SPSR
    // ELR
    // ttbr0_el1
    // ttbr1_el1
    // ----BOT----

    mov x0, x29 // source << 16 | kind
    mrs x1, ESR_EL1
    mov x2, SP 

    mov x28, lr // Save link register

    bl handle_exception // Jump to handle_exception

    mov lr, x28 // Restore link register

    // No ret because context restore

.global context_restore
context_restore:
    ldp x20, x21, [sp], #16 // Restore spsr_el1, elr_el1
    ldp x22, x23, [sp], #16 // Restore tpidr_el0, sp_el0
    ldp x24, x25, [sp], #16 // Restore ttbr0_el1, ttbr1_el1

    // Restore special registers
    msr ttbr1_el1, x25
    msr ttbr0_el1, x24

    // Ensure memory accesses have completed
    dsb     ishst
    tlbi    vmalle1
    dsb     ish
    isb

    msr elr_el1, x23
    msr spsr_el1, x22
    msr sp_el0, x21
    msr tpidr_el0, x20

    // Restore q registers
    ldp q0, q1, [sp], #32
    ldp q2, q3, [sp], #32
    ldp q4, q5, [sp], #32
    ldp q6, q7, [sp], #32
    ldp q8, q9, [sp], #32
    ldp q10, q11, [sp], #32
    ldp q12, q13, [sp], #32
    ldp q14, q15, [sp], #32
    ldp q16, q17, [sp], #32
    ldp q18, q19, [sp], #32
    ldp q20, q21, [sp], #32
    ldp q22, q23, [sp], #32
    ldp q24, q25, [sp], #32
    ldp q26, q27, [sp], #32
    ldp q28, q29, [sp], #32
    ldp q30, q31, [sp], #32

    // Restore x registers
    ldp x0, x1, [sp], #16
    ldp x2, x3, [sp], #16
    ldp x4, x5, [sp], #16
    ldp x6, x7, [sp], #16
    ldp x8, x9, [sp], #16
    ldp x10, x11, [sp], #16
    ldp x12, x13, [sp], #16
    ldp x14, x15, [sp], #16
    ldp x16, x17, [sp], #16
    ldp x18, x19, [sp], #16
    ldp x20, x21, [sp], #16
    ldp x22, x23, [sp], #16
    ldp x24, x25, [sp], #16
    ldp x26, x27, [sp], #16

    ret

.macro HANDLER source, kind
    .align 7
    stp     lr, xzr, [SP, #-16]!
    stp     x28, x29, [SP, #-16]!
    
    mov     x29, \source
    movk    x29, \kind, LSL #16
    bl      context_save
    
    ldp     x28, x29, [SP], #16
    ldp     lr, xzr, [SP], #16
    eret
.endm
    
.align 11
.global vectors
vectors:
    // Current EL0
    HANDLER 0, 0
    HANDLER 0, 1
    HANDLER 0, 2
    HANDLER 0, 3

    // Current ELx
    HANDLER 1, 0
    HANDLER 1, 1
    HANDLER 1, 2
    HANDLER 1, 3

    // Lower AArch64
    HANDLER 2, 0
    HANDLER 2, 1
    HANDLER 2, 2
    HANDLER 2, 3

    // Lower AArch32
    HANDLER 3, 0
    HANDLER 3, 1
    HANDLER 3, 2
    HANDLER 3, 3

