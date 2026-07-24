// count_byte — lowering adversarial: unmodelled SIMD mnemonic in candidate body.
.global count_byte
.text
count_byte:
    fmov d0, d0
    mov x0, xzr
    ret
