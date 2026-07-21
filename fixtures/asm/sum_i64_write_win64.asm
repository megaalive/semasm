; sum_i64 — memory adversarial (Win64): write to buffer on a read-only leaf.
BITS 64
DEFAULT REL

global sum_i64

section .text
sum_i64:
    mov qword [rcx], 0
    xor eax, eax
    ret
