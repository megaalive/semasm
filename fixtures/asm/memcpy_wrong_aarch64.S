// memcpy — intentionally WRONG (AArch64): returns 0, never copies.
.global memcpy
.text
memcpy:
    mov x0, xzr
    ret
