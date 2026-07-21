; count_byte — count occurrences of `needle` in `buffer[0..length]`.
; Microsoft x64: rcx=buffer, rdx=length, r8=needle, returns rax.
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    xor eax, eax        ; count = 0
    test rdx, rdx
    jz .done
.loop:
    cmp byte [rcx], r8b
    jne .skip
    inc rax
.skip:
    inc rcx
    dec rdx
    jnz .loop
.done:
    ret
