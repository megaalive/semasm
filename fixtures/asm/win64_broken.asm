BITS 64
DEFAULT REL
; Broken Microsoft x64 function: calls with NO shadow space reserved and
; without 16-byte alignment (RSP%16 == 8 at the call), and reads
; below RSP (no red zone on Windows).
global broken_func
broken_func:
    call    .callee
    mov     rax, [rsp - 8]
    ret
.callee:
    ret
