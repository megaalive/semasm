; find_first_byte — first index of needle in buffer[0..length], or length if absent.
; SysV AMD64: rdi=buffer, rsi=length, rdx=needle, returns rax.
BITS 64
DEFAULT REL

global find_first_byte

section .text
find_first_byte:
    xor eax, eax
    test rsi, rsi
    jz .done
.loop:
    movzx ecx, byte [rdi]
    cmp cl, dl
    je .done
    inc rdi
    inc rax
    dec rsi
    jnz .loop
.done:
    ret
