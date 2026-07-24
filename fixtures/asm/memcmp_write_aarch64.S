// memcmp — memory adversarial: write to buffer a on a read-only leaf (AAPCS64).
.global memcmp
.text
memcmp:
    strb wzr, [x0]
    mov x0, xzr
    ret
