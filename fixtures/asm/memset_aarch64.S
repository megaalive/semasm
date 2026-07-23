// memset — fill buffer[0..length] with value; returns 0.
// AAPCS64: x0=buffer, x1=length, x2=value, returns x0.
.global memset
.text
memset:
    mov x3, x0
    cbz x1, .done
.loop:
    strb w2, [x3]
    add x3, x3, #1
    subs x1, x1, #1
    b.ne .loop
.done:
    mov x0, xzr
    ret
