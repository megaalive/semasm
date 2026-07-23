; wrong: counts matches but does not mutate the buffer.
BITS 64
DEFAULT REL

global replace_byte

section .text
replace_byte:
    xor eax, eax
    test rdx, rdx
    jz .done
.loop:
    movzx r10d, byte [rcx]
    cmp r10b, r8b
    jne .skip
    inc rax
.skip:
    inc rcx
    dec rdx
    jnz .loop
.done:
    ret
