# count_byte — intentionally WRONG (RV64): always returns length.
.global count_byte
.text
count_byte:
    mv a0, a1
    ret
