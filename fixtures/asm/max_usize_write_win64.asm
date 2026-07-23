; max_usize — memory adversarial (Win64): store via first arg.
BITS 64
DEFAULT REL

global max_usize

section .text
max_usize:
    mov qword [rcx], 0
    xor eax, eax
    ret
