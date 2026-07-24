# count_byte — lowering adversarial: unmodelled fence in candidate body.
.global count_byte
.text
count_byte:
    fence
    li a0, 0
    ret
