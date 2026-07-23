// memcmp — unsigned lexicographic compare a[0..length] vs b; return -1/0/1.
// AAPCS64: x0=a, x1=b, x2=length, returns x0 (isize).
.global memcmp
.text
memcmp:
    mov x3, xzr
    cbz x2, .done
.loop:
    ldrb w4, [x0]
    ldrb w5, [x1]
    cmp w4, w5
    b.ne .diff
    add x0, x0, #1
    add x1, x1, #1
    subs x2, x2, #1
    b.ne .loop
    mov x0, xzr
    ret
.diff:
    b.lo .lt
    mov x0, #1
    ret
.lt:
    mov x0, xzr
    sub x0, x0, #1
    ret
.done:
    mov x0, xzr
    ret
