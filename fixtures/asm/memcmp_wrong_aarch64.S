// memcmp — intentionally WRONG (AArch64): always returns 0.
.global memcmp
.text
memcmp:
    mov x0, xzr
    ret
