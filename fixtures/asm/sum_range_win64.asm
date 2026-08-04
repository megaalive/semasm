; Return the wrapping triangular sum 0+1+...+n.
; Microsoft x64: rcx=n, returns rax.
BITS 64
DEFAULT REL

global sum_range

section .text
sum_range:
    xor eax, eax
    xor edx, edx
.loop:
    add rax, rdx
    inc rdx
    cmp rdx, rcx
    jle .loop
    ret
