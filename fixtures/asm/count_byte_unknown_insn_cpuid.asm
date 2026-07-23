; count_byte — lowering adversarial: a second unmodelled-mnemonic class.
; `count_byte_unknown_insn.asm` covers the AVX/SIMD-state class (`vzeroupper`);
; this twin covers the privileged/CPU-identification class (`cpuid`) so the
; lowering gap corpus is not resting on a single mnemonic family (Dx).
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    cpuid
    xor eax, eax
    ret
