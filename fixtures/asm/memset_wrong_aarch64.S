// memset — intentionally WRONG (AArch64): returns 0, never stores.
.global memset
.text
memset:
    mov x0, xzr
    ret
