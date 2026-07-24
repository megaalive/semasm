# find_last_byte — intentionally WRONG (RV64): always returns 0.
.global find_last_byte
.text
find_last_byte:
    li a0, 0
    ret
