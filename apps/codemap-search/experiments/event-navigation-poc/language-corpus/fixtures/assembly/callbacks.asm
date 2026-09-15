PrimaryTable:
    dw CallbackA ; store_primary
SecondaryTable:
    dw CallbackB ; store_secondary
OtherTable:
    dw CallbackC
FirePrimary:
    ld hl, PrimaryTable
    ld a, [hli]
    ld h, [hl]
    ld l, a
    jp hl ; call_primary
FireSecondary:
    ld hl, SecondaryTable
    ld a, [hli]
    ld h, [hl]
    ld l, a
    jp hl ; call_secondary
FireOther:
    ld hl, OtherTable
    ld a, [hli]
    ld h, [hl]
    ld l, a
    jp hl ; call_other
CallbackA:
    ret
CallbackB:
    ret
CallbackC:
    ret
