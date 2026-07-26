; max_i64 — signed max via cmov (no branch).
; Microsoft x64: rcx=a, rdx=b, returns rax.
; Proves SemASM models cmovg as Select (not Unknown) under require_complete_lowering.
BITS 64
DEFAULT REL

global max_i64

section .text
max_i64:
    mov rax, rcx
    cmp rdx, rax
    cmovg rax, rdx
    ret
