; count_byte -- ABI adversarial (Win64): write callee-saved RBX without save/restore.
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    mov rbx, rcx
    xor eax, eax
    ret
