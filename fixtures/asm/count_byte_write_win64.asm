; count_byte — memory adversarial (Win64): write to buffer on a read-only leaf.
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    mov byte [rcx], 0
    xor eax, eax
    ret
