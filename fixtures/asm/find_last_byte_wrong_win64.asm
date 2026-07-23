; find_last_byte -- intentionally WRONG (Win64): always returns 0.
BITS 64
DEFAULT REL

global find_last_byte

section .text
find_last_byte:
    xor eax, eax
    ret
