; count_byte -- ABI adversarial (Win64): unbalanced stack (push without pop).
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    push rax
    ret
