; max_usize — ABI adversarial: write callee-saved RBX without save/restore.
BITS 64
DEFAULT REL

global max_usize

section .text
max_usize:
    mov rbx, rdi
    xor eax, eax
    ret
