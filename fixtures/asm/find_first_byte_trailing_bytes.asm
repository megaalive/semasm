; find_first_byte — decode adversarial: trailing undecoded junk after a valid
; ret, on a different leaf/contract shape than `count_byte_trailing_bytes.asm`
; (buffer-scan-with-early-exit, not accumulate-and-loop) so the decode-gap
; corpus is not only exercised through one leaf family (Dx).
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
    ; Incomplete/invalid trailing prefix bytes Capstone cannot consume fully.
    db 0x66, 0x67, 0xf0
