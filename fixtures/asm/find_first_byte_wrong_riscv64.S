# find_first_byte — intentionally WRONG (RV64): always returns 0.
.global find_first_byte
.text
find_first_byte:
    li a0, 0
    ret
