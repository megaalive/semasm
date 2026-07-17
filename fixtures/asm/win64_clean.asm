BITS 64
DEFAULT REL
; Clean Microsoft x64 leaf function: reserves 40 bytes (32 shadow + 8 to
; reach 16-byte alignment), makes a call, restores the stack, and returns.
; At the `call` RSP%16 == 0 and 40 bytes are reserved (>= 32 shadow).
global clean_func
clean_func:
    sub     rsp, 40
    mov     ecx, 1
    mov     edx, 2
    mov     r8, 3
    mov     r9, 4
    call    .callee
    add     rsp, 40
    ret
.callee:
    ret
