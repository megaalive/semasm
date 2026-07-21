; count_byte with forbidden syscall in the candidate body (capability fail).
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    syscall
    ret
