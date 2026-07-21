// count_byte with forbidden svc in the candidate body (capability fail).
.global count_byte
.text
count_byte:
    mov x8, #93
    svc #0
    ret
