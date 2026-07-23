; memcmp -- memory adversarial (Win64): write to buffer a on a read-only leaf.
BITS 64
DEFAULT REL

global memcmp

section .text
memcmp:
    mov byte [rcx], 0
    xor eax, eax
    ret
