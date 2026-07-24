# count_byte — ABI adversarial: unbalanced stack (sp-relative store without restore).
# Prefer sd/-disp over addi: Capstone may compress addi sp into c.addi16sp forms
# that the walker historically missed; sd -16(sp) is already modelled as push.
.global count_byte
.text
count_byte:
    sd zero, -16(sp)
    li a0, 0
    ret
