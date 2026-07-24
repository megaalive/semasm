// count_byte — object policy adversarial: writable+executable section (W+X).
// Use a non-.text name: gas/clang refuse or strip SHF_WRITE on .text (flags 0x6).
.section .semasm_wx,"awx",@progbits
.global count_byte
count_byte:
    mov x0, xzr
    ret
