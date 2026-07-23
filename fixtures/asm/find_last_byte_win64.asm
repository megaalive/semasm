; find_last_byte -- last index of needle in buffer[0..length], or length if absent.
; Microsoft x64: rcx=buffer, rdx=length, r8=needle, returns rax.
BITS 64
DEFAULT REL

global find_last_byte

section .text
find_last_byte:
    mov rax, rdx
    test rdx, rdx
    jz .done
    xor r9d, r9d
.loop:
    cmp byte [rcx], r8b
    jne .skip
    mov rax, r9
.skip:
    inc rcx
    inc r9
    dec rdx
    jnz .loop
.done:
    ret
