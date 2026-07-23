; find_last_byte -- last index of needle in buffer[0..length], or length if absent.
; SysV AMD64: rdi=buffer, rsi=length, rdx=needle, returns rax.
BITS 64
DEFAULT REL

global find_last_byte

section .text
find_last_byte:
    mov rax, rsi
    test rsi, rsi
    jz .done
    xor ecx, ecx
.loop:
    movzx r8d, byte [rdi]
    cmp r8b, dl
    jne .skip
    mov rax, rcx
.skip:
    inc rdi
    inc rcx
    dec rsi
    jnz .loop
.done:
    ret
