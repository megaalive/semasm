// count_byte — lowering adversarial: unmodelled system-register MRS.
.global count_byte
.text
count_byte:
    mrs x0, nzcv
    ret
