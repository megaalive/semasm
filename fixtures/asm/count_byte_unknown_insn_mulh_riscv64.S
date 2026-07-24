# count_byte — lowering adversarial: unmodelled M-extension mulh.
.global count_byte
.text
count_byte:
    mulh a0, a0, a1
    ret
