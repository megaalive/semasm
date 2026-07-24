// count_byte — memory adversarial: write to buffer on a read-only leaf (AAPCS64).
.global count_byte
.text
count_byte:
    strb wzr, [x0]
    mov x0, xzr
    ret
