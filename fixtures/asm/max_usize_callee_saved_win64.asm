; max_usize — ABI adversarial (Win64): write callee-saved RBX without save/restore.
BITS 64
DEFAULT REL

global max_usize

section .text
max_usize:
    mov rbx, rcx
    xor eax, eax
    ret
