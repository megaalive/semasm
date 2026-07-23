; memcpy — copy src[0..length] into dst[0..length]; returns 0.
; Microsoft x64: rcx=dst, rdx=src, r8=length, returns rax.
BITS 64
DEFAULT REL

global memcpy

section .text
memcpy:
    xor eax, eax
    test r8, r8
    jz .done
.loop:
    mov r9b, byte [rdx]
    mov byte [rcx], r9b
    inc rcx
    inc rdx
    dec r8
    jnz .loop
.done:
    ret
