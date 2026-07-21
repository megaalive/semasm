// count_byte — intentionally WRONG (AArch64): always returns length.
.global count_byte
.text
count_byte:
    mov x0, x1
    ret
