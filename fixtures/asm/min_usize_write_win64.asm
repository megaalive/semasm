; min_usize — memory adversarial (Win64): store via first arg.
BITS 64
DEFAULT REL

global min_usize

section .text
min_usize:
    mov qword [rcx], 0
    xor eax, eax
    ret
