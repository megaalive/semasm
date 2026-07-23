; memcpy — copy src[0..length] into dst[0..length]; returns 0.
; SysV AMD64: rdi=dst, rsi=src, rdx=length, returns rax.
BITS 64
DEFAULT REL

global memcpy

section .text
memcpy:
    xor eax, eax
    test rdx, rdx
    jz .done
.loop:
    mov cl, byte [rsi]
    mov byte [rdi], cl
    inc rdi
    inc rsi
    dec rdx
    jnz .loop
.done:
    ret
