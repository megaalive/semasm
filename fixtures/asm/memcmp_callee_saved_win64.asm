; memcmp -- ABI adversarial (Win64): write callee-saved RBX without save/restore.
BITS 64
DEFAULT REL

global memcmp

section .text
memcmp:
    mov rbx, rcx
    xor eax, eax
    ret
