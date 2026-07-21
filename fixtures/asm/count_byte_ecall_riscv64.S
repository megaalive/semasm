# count_byte with forbidden ecall in the candidate body (capability fail).
.global count_byte
.text
count_byte:
    li a7, 93
    ecall
    ret
