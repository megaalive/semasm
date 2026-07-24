# find_first_byte — first index of needle in buffer[0..length], or length if absent.
# RISC-V LP64: a0=buffer, a1=length, a2=needle, returns a0.
.global find_first_byte
.text
find_first_byte:
    mv t0, a0
    mv t1, a1
    mv t2, a2
    li t3, 0
    beq t1, zero, .done
.loop:
    lbu t4, 0(t0)
    beq t4, t2, .found
    addi t0, t0, 1
    addi t3, t3, 1
    addi t1, t1, -1
    bne t1, zero, .loop
.done:
    mv a0, t3
    ret
.found:
    mv a0, t3
    ret
