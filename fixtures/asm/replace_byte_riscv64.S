# replace_byte — replace needle with replacement in buffer[0..length]; return count.
# RISC-V LP64: a0=buffer, a1=length, a2=needle, a3=replacement, returns a0.
.global replace_byte
.text
replace_byte:
    mv t0, a0
    mv t1, a1
    mv t2, a2
    mv t3, a3
    li t4, 0
    beq t1, zero, .done
.loop:
    lbu t5, 0(t0)
    bne t5, t2, .skip
    sb t3, 0(t0)
    addi t4, t4, 1
.skip:
    addi t0, t0, 1
    addi t1, t1, -1
    bne t1, zero, .loop
.done:
    mv a0, t4
    ret
