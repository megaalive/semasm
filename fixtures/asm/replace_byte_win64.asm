; replace_byte — replace needle with replacement in buffer[0..length]; return count.
; Microsoft x64: rcx=buffer, rdx=length, r8=needle, r9=replacement, returns rax.
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
    mov byte [rcx], r9b
    inc rax
.skip:
    inc rcx
    dec rdx
    jnz .loop
.done:
    ret
