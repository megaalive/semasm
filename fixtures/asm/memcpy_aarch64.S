// memcpy — copy src[0..length] into dst[0..length]; returns 0.
// AAPCS64: x0=dst, x1=src, x2=length, returns x0.
.global memcpy
.text
memcpy:
    mov x3, x0
    mov x4, x1
    cbz x2, .done
.loop:
    ldrb w5, [x4]
    strb w5, [x3]
    add x3, x3, #1
    add x4, x4, #1
    subs x2, x2, #1
    b.ne .loop
.done:
    mov x0, xzr
    ret
