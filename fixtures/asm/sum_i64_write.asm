; sum_i64 — memory adversarial: write to buffer on a read-only leaf.
BITS 64
DEFAULT REL

global sum_i64

section .text
sum_i64:
    mov qword [rdi], 0
    xor eax, eax
    ret
