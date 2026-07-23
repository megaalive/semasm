; find_last_byte — decode adversarial: trailing undecoded junk after ret (H4).
; Third leaf/contract shape (after count_byte + find_first_byte) so the
; trailing-bytes corpus spans accumulate / early-exit / last-index scans.
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
    db 0x66, 0x67, 0xf0
