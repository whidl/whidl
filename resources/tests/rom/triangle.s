.section .text
.global _start

_start:
    mov r1, #5
    mov r0, #0

loop:
    cmp r1, #0
    beq end
    add r0, r1, r0
    sub r1, r1, #1
    b loop

end:
    b end

.end
