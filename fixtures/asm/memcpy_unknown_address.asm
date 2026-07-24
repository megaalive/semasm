; memcpy_unknown_address — uses [rax] without modeling rax as a param.
; Expected Region/Alias evidence: incomplete (unknown_memory_accesses > 0).
BITS 64
DEFAULT REL

global memcpy

section .text
memcpy:
    xor eax, eax
    ; Deliberate unmodeled address: rax is not a pointer parameter.
    mov cl, byte [rax]
    ret
