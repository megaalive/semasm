// count_byte — ABI adversarial: unbalanced stack (sub sp without restore).
.global count_byte
.text
count_byte:
    sub sp, sp, #16
    mov x0, xzr
    ret
