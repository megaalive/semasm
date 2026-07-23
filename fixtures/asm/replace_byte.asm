; replace_byte — replace needle with replacement in buffer[0..length]; return count.
; SysV AMD64: rdi=buffer, rsi=length, rdx=needle, rcx=replacement, returns rax.
BITS 64
DEFAULT REL

global replace_byte

section .text
replace_byte:
    xor eax, eax
    test rsi, rsi
    jz .done
.loop:
    movzx r8d, byte [rdi]
    cmp r8b, dl
    jne .skip
    mov byte [rdi], cl
    inc rax
.skip:
    inc rdi
    dec rsi
    jnz .loop
.done:
    ret
