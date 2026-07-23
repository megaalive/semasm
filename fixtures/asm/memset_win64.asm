; memset — fill buffer[0..length] with value; returns 0.
; Microsoft x64: rcx=buffer, rdx=length, r8=value, returns rax.
BITS 64
DEFAULT REL

global memset

section .text
memset:
    xor eax, eax
    test rdx, rdx
    jz .done
.loop:
    mov byte [rcx], r8b
    inc rcx
    dec rdx
    jnz .loop
.done:
    ret
