; count_byte — capability adversarial (Win64): forbidden syscall in the candidate body.
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    syscall
    10|    ret
