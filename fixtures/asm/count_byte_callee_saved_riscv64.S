# count_byte — ABI adversarial: write callee-saved s1 without save/restore.
.global count_byte
.text
count_byte:
    mv s1, a0
    li a0, 0
    ret
