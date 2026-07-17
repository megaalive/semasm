; exit.asm — minimal Linux x86-64 exit(42)
;
; Build:
;   nasm -f elf64 exit.asm -o exit.o
;   ld.lld exit.o -o exit
;
; Run:
;   ./exit; echo $?
;   42

section .text
global _start

_start:
    mov     eax, 60         ; __NR_exit (x86-64 Linux)
    mov     edi, 42         ; exit status
    syscall
