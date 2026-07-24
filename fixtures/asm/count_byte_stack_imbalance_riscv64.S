# count_byte — ABI adversarial: unbalanced stack (addi sp without restore).
.global count_byte
.text
count_byte:
    addi sp, sp, -16
    li a0, 0
    ret
