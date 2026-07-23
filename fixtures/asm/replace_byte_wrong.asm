; wrong: counts matches but does not mutate the buffer.
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
    inc rax
.skip:
    inc rdi
    dec rsi
    jnz .loop
.done:
    ret
