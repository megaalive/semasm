# find_last_byte — last index of needle in buffer[0..length], or length if absent.
# RISC-V LP64: a0=buffer, a1=length, a2=needle, returns a0.
.global find_last_byte
.text
find_last_byte:
    mv t0, a0
    mv t1, a1
    mv t2, a2
    mv t3, a1
    li t4, 0
    beq t1, zero, .done
.loop:
    lbu t5, 0(t0)
    bne t5, t2, .skip
    mv t3, t4
.skip:
    addi t0, t0, 1
    addi t4, t4, 1
    addi t1, t1, -1
    bne t1, zero, .loop
.done:
    mv a0, t3
    ret
