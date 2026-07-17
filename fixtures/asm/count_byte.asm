; count_byte — count occurrences of `needle` in `buffer[0..length]`.
; SysV AMD64: rdi=buffer, rsi=length, rdx=needle, returns rax.
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    xor eax, eax        ; count = 0
    test rsi, rsi
    jz .done
.loop:
    cmp byte [rdi], dl
    jne .skip
    inc rax
.skip:
    inc rdi
    dec rsi
    jnz .loop
.done:
    ret
