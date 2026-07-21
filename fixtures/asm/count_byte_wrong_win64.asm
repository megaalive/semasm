; count_byte — intentionally WRONG (Win64): always returns length.
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    mov rax, rdx
    ret
