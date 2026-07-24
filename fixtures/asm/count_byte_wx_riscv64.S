# count_byte — object policy adversarial: writable+executable .text (W+X).
.section .text,"awx",@progbits
.global count_byte
count_byte:
    li a0, 0
    ret
