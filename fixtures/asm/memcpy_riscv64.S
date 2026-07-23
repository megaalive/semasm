# memcpy — copy src[0..length] into dst[0..length]; returns 0.
# RISC-V LP64: a0=dst, a1=src, a2=length, returns a0.
.global memcpy
.text
memcpy:
    mv t0, a0
    mv t1, a1
    mv t2, a2
    beq t2, zero, .done
.loop:
    lbu t3, 0(t1)
    sb t3, 0(t0)
    addi t0, t0, 1
    addi t1, t1, 1
    addi t2, t2, -1
    bne t2, zero, .loop
.done:
    li a0, 0
    ret
