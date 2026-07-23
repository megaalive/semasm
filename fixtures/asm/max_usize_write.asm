; max_usize — memory adversarial: store via first arg (pure-int leaf must stay read-only).
BITS 64
DEFAULT REL

global max_usize

section .text
max_usize:
    mov qword [rdi], 0
    xor eax, eax
    ret
