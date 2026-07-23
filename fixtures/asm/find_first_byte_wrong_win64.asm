; find_first_byte — intentionally WRONG (Win64): always returns 0.
BITS 64
DEFAULT REL

global find_first_byte

section .text
find_first_byte:
    xor eax, eax
    ret
