# memcmp — unsigned lexicographic compare a[0..length] vs b; return -1/0/1.
# RISC-V LP64: a0=a, a1=b, a2=length, returns a0 (isize).
.global memcmp
.text
memcmp:
    mv t0, a0
    mv t1, a1
    mv t2, a2
    li t3, 0
    beq t2, zero, .done
.loop:
    lbu t4, 0(t0)
    lbu t5, 0(t1)
    bne t4, t5, .diff
    addi t0, t0, 1
    addi t1, t1, 1
    addi t2, t2, -1
    bne t2, zero, .loop
    li a0, 0
    ret
.diff:
    bltu t4, t5, .lt
    li a0, 1
    ret
.lt:
    li a0, -1
    ret
.done:
    mv a0, t3
    ret
