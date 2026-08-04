; Deliberately wrong: computes 0+1+...+(n-1), excluding n.
; Microsoft x64: rcx=n, returns rax.
BITS 64
DEFAULT REL

global sum_range

section .text
sum_range:
    xor eax, eax
    xor edx, edx
.loop:
    cmp rdx, rcx
    jge .done
    add rax, rdx
    inc rdx
    jmp .loop
.done:
    ret
