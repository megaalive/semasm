# memcmp — intentionally WRONG (RV64): always returns 0.
.global memcmp
.text
memcmp:
    li a0, 0
    ret
