; memcmp -- intentionally WRONG: always returns 0.
BITS 64
DEFAULT REL

global memcmp

section .text
memcmp:
    xor eax, eax
    ret
