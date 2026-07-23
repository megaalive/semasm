; find_first_byte — first index of needle in buffer[0..length], or length if absent.
; Microsoft x64: rcx=buffer, rdx=length, r8=needle, returns rax.
BITS 64
DEFAULT REL

global find_first_byte

section .text
find_first_byte:
    xor eax, eax
    test rdx, rdx
    jz .done
.loop:
    cmp byte [rcx], r8b
    je .done
    inc rcx
    inc rax
    dec rdx
    jnz .loop
.done:
    ret
