; min_usize — ABI adversarial: write callee-saved RBX without save/restore.
BITS 64
DEFAULT REL

global min_usize

section .text
min_usize:
    mov rbx, rdi
    xor eax, eax
    ret
