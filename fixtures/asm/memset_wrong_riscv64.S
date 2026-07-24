# memset — intentionally WRONG (RV64): returns 0, never stores.
.global memset
.text
memset:
    li a0, 0
    ret
