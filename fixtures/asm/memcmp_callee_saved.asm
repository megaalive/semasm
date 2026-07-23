; memcmp -- ABI adversarial: write callee-saved RBX without save/restore.
BITS 64
DEFAULT REL

global memcmp

section .text
memcmp:
    mov rbx, rdi
    xor eax, eax
    ret
