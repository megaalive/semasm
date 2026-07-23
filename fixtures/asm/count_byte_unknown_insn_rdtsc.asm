; count_byte — lowering adversarial: a third unmodelled-mnemonic class (H4).
; Prior twins: AVX/SIMD (`vzeroupper`), privileged/CPU-id (`cpuid`).
; This twin covers timestamp/counter class (`rdtsc`) so the unknown-insn
; corpus is not resting on two families alone. decode/lower stay `partial`.
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    rdtsc
    xor eax, eax
    ret
