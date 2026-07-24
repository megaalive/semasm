// count_byte — object policy adversarial: writable+executable .text (W+X).
.section .text,"awx",@progbits
.global count_byte
count_byte:
    mov x0, xzr
    ret
