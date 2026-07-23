; memcmp -- memory adversarial: write to buffer a on a read-only leaf.
BITS 64
DEFAULT REL

global memcmp

section .text
memcmp:
    mov byte [rdi], 0
    xor eax, eax
    ret
