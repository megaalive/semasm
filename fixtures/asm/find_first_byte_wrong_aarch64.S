// find_first_byte — intentionally WRONG (AArch64): always returns 0.
.global find_first_byte
.text
find_first_byte:
    mov x0, xzr
    ret
