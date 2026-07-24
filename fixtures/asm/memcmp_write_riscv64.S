# memcmp — memory adversarial: write to buffer a on a read-only leaf (RV LP64).
.global memcmp
.text
memcmp:
    sb zero, 0(a0)
    li a0, 0
    ret
