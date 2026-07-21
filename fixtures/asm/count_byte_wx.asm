; count_byte — object policy adversarial: writable+executable .text (W+X).
BITS 64
DEFAULT REL

section .text exec write
global count_byte

count_byte:
    xor eax, eax
    ret
