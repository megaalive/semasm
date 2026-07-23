; count_byte — object policy adversarial (Win64): writable+executable .text (W+X).
; Honesty: NASM win64 does not emit IMAGE_SCN_MEM_WRITE on code sections even when
; `write` is requested. Gate evidence uses the patched COFF fixture
; `fixtures/obj/count_byte_wx_win64.obj` (WRITE|EXECUTE) for object-policy fail-closed.
BITS 64
DEFAULT REL

section .text exec write
global count_byte

count_byte:
    xor eax, eax
    ret
