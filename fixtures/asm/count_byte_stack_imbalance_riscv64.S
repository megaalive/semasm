# count_byte — ABI adversarial: unbalanced stack (addi sp without restore).
# Force uncompressed addi so Capstone sees addi rd, rs1, imm (not c.addi16sp).
.option norvc
.global count_byte
.text
count_byte:
    addi sp, sp, -16
    li a0, 0
    ret
