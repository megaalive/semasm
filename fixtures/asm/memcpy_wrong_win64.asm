; wrong: returns 0 (status honored) but never copies src into dst.
BITS 64
DEFAULT REL

global memcpy

section .text
memcpy:
    xor eax, eax
    ret
