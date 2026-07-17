BITS 64
DEFAULT REL

EXTERN GetStdHandle
EXTERN WriteFile
EXTERN ExitProcess

section .data
    msg db "SemASM Windows x64", 13, 10
    msg_len equ $ - msg

section .bss
    written resq 1

section .text
global main
main:
    ; hStdOut = GetStdHandle(STD_OUTPUT_HANDLE = -11)
    sub     rsp, 40
    mov     ecx, -11
    call    GetStdHandle
    mov     rcx, rax           ; hFile
    mov     rdx, msg           ; lpBuffer
    mov     r8,  msg_len      ; nNumberOfBytesToWrite
    mov     r9,  written      ; lpNumberOfBytesWritten
    mov     qword [rsp + 32], 0  ; lpOverlapped = NULL (shadow slot 5th arg)
    call    WriteFile
    add     rsp, 40

    ; ExitProcess(0)
    sub     rsp, 40
    mov     ecx, 0
    call    ExitProcess

section .drectve comment linked by lld-link for imports
; (imports resolved through EXTERN above; lld-link auto-links kernel32)
