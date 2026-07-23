; sum_i64 — ABI adversarial (Win64): write callee-saved RBX without save/restore.
BITS 64
DEFAULT REL

global sum_i64

section .text
sum_i64:
    mov rbx, rcx
    xor eax, eax
    ret
