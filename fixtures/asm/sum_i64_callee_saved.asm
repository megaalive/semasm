; sum_i64 — ABI adversarial: write callee-saved RBX without save/restore.
BITS 64
DEFAULT REL

global sum_i64

section .text
sum_i64:
    mov rbx, rdi
    xor eax, eax
    ret
