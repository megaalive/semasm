; memcmp -- unsigned lexicographic compare a[0..length] vs b; return -1/0/1.
; Microsoft x64: rcx=a, rdx=b, r8=length, returns rax (isize).
BITS 64
DEFAULT REL

global memcmp

section .text
memcmp:
    xor eax, eax
    test r8, r8
    jz .done
.loop:
    movzx r9d, byte [rcx]
    movzx r10d, byte [rdx]
    cmp r9d, r10d
    jne .diff
    inc rcx
    inc rdx
    dec r8
    jnz .loop
    ret
.diff:
    jb .lt
    xor eax, eax
    inc eax
    ret
.lt:
    xor eax, eax
    dec rax
.done:
    ret
