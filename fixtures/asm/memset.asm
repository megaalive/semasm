; memset — fill buffer[0..length] with value; returns 0.
; SysV AMD64: rdi=buffer, rsi=length, rdx=value, returns rax.
BITS 64
DEFAULT REL

global memset

section .text
memset:
    xor eax, eax
    test rsi, rsi
    jz .done
.loop:
    mov byte [rdi], dl
    inc rdi
    dec rsi
    jnz .loop
.done:
    ret
