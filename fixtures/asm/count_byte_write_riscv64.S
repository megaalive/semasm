# count_byte — memory adversarial: write to buffer on a read-only leaf (RV LP64).
.global count_byte
.text
count_byte:
    sb zero, 0(a0)
    li a0, 0
    ret
