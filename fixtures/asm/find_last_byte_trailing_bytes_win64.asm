; find_last_byte — decode adversarial (Win64): trailing junk after ret (H4).
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
    movzx r10d, byte [rcx]
    cmp r10b, r8b
    jne .skip
    mov rax, r9
.skip:
    inc rcx
    inc r9
    dec rdx
    jnz .loop
.done:
    ret
    db 0x66, 0x67, 0xf0
