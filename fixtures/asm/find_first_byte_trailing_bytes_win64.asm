; find_first_byte — decode adversarial (Win64): trailing undecoded junk after
; a valid ret, on a different leaf/contract shape than
; `count_byte_trailing_bytes_win64.asm` (buffer-scan-with-early-exit, not
; accumulate-and-loop) so the decode-gap corpus is not only exercised through
; one leaf family (Dx).
; Microsoft x64: rcx=buffer, rdx=length, r8=needle, returns rax.
BITS 64
DEFAULT REL

global find_first_byte

section .text
find_first_byte:
    xor eax, eax
    test rdx, rdx
    jz .done
.loop:
    cmp byte [rcx], r8b
    je .done
    inc rcx
    inc rax
    dec rdx
    jnz .loop
.done:
    ret
    ; Incomplete/invalid trailing prefix bytes Capstone cannot consume fully.
    db 0x66, 0x67, 0xf0
