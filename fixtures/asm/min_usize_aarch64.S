// min_usize — return the smaller of two unsigned sizes.
// AAPCS64: x0=a, x1=b, returns x0.
.global min_usize
.text
min_usize:
    cmp x0, x1
    b.le .a_smaller_or_equal
    mov x0, x1
    ret
.a_smaller_or_equal:
    ret
