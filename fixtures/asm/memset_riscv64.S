# memset — fill buffer[0..length] with value; returns 0.
# RISC-V LP64: a0=buffer, a1=length, a2=value, returns a0.
.global memset
.text
memset:
    mv t0, a0
    mv t1, a1
    mv t2, a2
    beq t1, zero, .done
.loop:
    sb t2, 0(t0)
    addi t0, t0, 1
    addi t1, t1, -1
    bne t1, zero, .loop
.done:
    li a0, 0
    ret
