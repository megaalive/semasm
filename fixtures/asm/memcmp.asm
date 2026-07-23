; memcmp -- unsigned lexicographic compare a[0..length] vs b; return -1/0/1.
; SysV AMD64: rdi=a, rsi=b, rdx=length, returns rax (isize).
BITS 64
DEFAULT REL

global memcmp

section .text
memcmp:
    xor eax, eax
    test rdx, rdx
    jz .done
.loop:
    movzx ecx, byte [rdi]
    movzx r8d, byte [rsi]
    cmp ecx, r8d
    jb .lt
    ja .gt
    inc rdi
    inc rsi
    dec rdx
    jnz .loop
    ret
.lt:
    mov rax, -1
    ret
.gt:
    mov rax, 1
.done:
    ret
