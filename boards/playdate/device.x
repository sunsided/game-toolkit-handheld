/* Device ELF layout for pdc when linking with rust-lld instead of the ARM GNU toolchain:
   one RWX load segment holding code, data, bss and the relocations pdc applies at load. */
ENTRY(eventHandlerShim)

PHDRS { text PT_LOAD FLAGS(7); }

SECTIONS
{
	.text : { *(.text .text.*) *(.rodata .rodata.*) } :text
	.data : { *(.data .data.*) } :text
	.bss : { . = ALIGN(4); *(.bss .bss.*) *(COMMON) . = ALIGN(4); } :text
	.reloc : { . = ALIGN(4); *(.rel*.*) } :text
	/DISCARD/ : { *(.ARM.exidx*) }
}
