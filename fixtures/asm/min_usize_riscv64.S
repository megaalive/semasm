# min_usize — return the smaller of two unsigned sizes.
# RISC-V LP64: a0=a, a1=b, returns a0.
.global min_usize
.text
min_usize:
    bge a0, a1, .a_smaller_or_equal
    mv a0, a1
    ret
.a_smaller_or_equal:
    ret
