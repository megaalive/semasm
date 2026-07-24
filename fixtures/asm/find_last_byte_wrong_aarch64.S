// find_last_byte — intentionally WRONG (AArch64): always returns 0.
.global find_last_byte
.text
find_last_byte:
    mov x0, xzr
    ret
