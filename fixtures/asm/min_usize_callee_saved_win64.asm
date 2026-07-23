; min_usize — ABI adversarial (Win64): write callee-saved RBX without save/restore.
BITS 64
DEFAULT REL

global min_usize

section .text
min_usize:
    mov rbx, rcx
    xor eax, eax
    ret
