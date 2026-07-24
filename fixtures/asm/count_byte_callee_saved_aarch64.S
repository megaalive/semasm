// count_byte — ABI adversarial: write callee-saved x19 without save/restore.
.global count_byte
.text
count_byte:
    mov x19, x0
    mov x0, xzr
    ret
