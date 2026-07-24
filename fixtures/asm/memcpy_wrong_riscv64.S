# memcpy — intentionally WRONG (RV64): returns 0, never copies.
.global memcpy
.text
memcpy:
    li a0, 0
    ret
