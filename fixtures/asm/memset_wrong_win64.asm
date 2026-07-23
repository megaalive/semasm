; wrong: returns 0 (status honored) but never stores to the buffer.
BITS 64
DEFAULT REL

global memset

section .text
memset:
    xor eax, eax
    ret
