ENTRY(_start)

SECTIONS
{

    . = 0xffffffff80000000 + 1M;

    .boot : {
        *(.boot)
    }

    .text ALIGN(4K) : {
        *(.text)
    }

    .rodata ALIGN(4K) : {
        *(.rodata)
    }

    .data ALIGN(4K) : {
        *(.data)
    }

    .bss ALIGN(4K) : {
        *(.bss)
        *(COMMON)
    }

    .init_stack : {
        *(.init_stack)
    }

    .eh_frame : {
        *(.eh_frame)
    }

    .gcc_except_table : {
        *(.gcc_except_table)
    }

    .debug_info : {
        *(.debug_info)
    }

    .debug_line : {
        *(.debug_line)
    }

    .debug_str : {
        *(.debug_str)
    }

    .comment : {
        *(.comment)
    }

    .note.gnu.build-id : {
        *(.note.gnu.build-id)
    }

    .symtab : {
        *(.symtab)
    }

    .strtab : {
        *(.strtab)
    }

    .shstrtab : {
        *(.shstrtab)
    }

    .rela.text : {
        *(.rela.text)
    }

    .rela.data : {
        *(.rela.data)
    }

    .rela.rodata : {
        *(.rela.rodata)
    }

    .rela.bss : {
        *(.rela.bss)
    }

    .rela.init_array : {
        *(.rela.init_array)
    }

    .rela.fini_array : {
        *(.rela.fini_array)
    }

    .init_array : {
        *(.init_array)
    }

    .fini_array : {
        *(.fini_array)
    }

    .ctors : {
        *(.ctors)
    }

    .dtors : {
        *(.dtors)
    }

    .got : {
        *(.got)
    }

    .got.plt : {
        *(.got.plt)
    }

    .plt : {
        *(.plt)
    }

    .interp : {
        *(.interp)
    }

    .dynamic : {
        *(.dynamic)
    }

    .hash : {
        *(.hash)
    }

    .dynsym : {
        *(.dynsym)
    }

    .dynstr : {
        *(.dynstr)
    }

    .gnu.version : {
        *(.gnu.version)
    }

    .gnu.version_d : {
        *(.gnu.version_d)
    }

    .gnu.version_r : {
        *(.gnu.version_r)
    }

    .rela.dyn : {
        *(.rela.dyn)
    }

    .rela.plt : {
        *(.rela.plt)
    }

    .note.ABI-tag : {
        *(.note.ABI-tag)
    }

    .note.gnu.gold-version : {
        *(.note.gnu.gold-version)
    }

    .gnu.hash : {
        *(.gnu.hash)
    }

    .ARM.extab : {
        *(.ARM.extab)
    }

    .ARM.exidx : {
        *(.ARM.exidx)
    }

    .pdr : {
        *(.pdr)
    }

    .xt.prop : {
        *(.xt.prop)
    }

    .xt.lit : {
        *(.xt.lit)
    }

    .stab : {
        *(.stab)
    }

    .stabstr : {
        *(.stabstr)
    }

    .stab.excl : {
        *(.stab.excl)
    }

    .stab.exclstr : {
        *(.stab.exclstr)
    }

    .stab.index : {
        *(.stab.index)
    }

    .stab.indexstr : {
        *(.stab.indexstr)
    }

    .comment : {
        *(.comment)
    }

    .debug : {
        *(.debug)
    }

    .line : {
        *(.line)
    }

    .debug_srcinfo : {
        *(.debug_srcinfo)
    }

    .debug_sfnames : {
        *(.debug_sfnames)
    }

    .debug_aranges : {
        *(.debug_aranges)
    }

    .debug_pubnames : {
        *(.debug_pubnames)
    }

    .debug_pubtypes : {
        *(.debug_pubtypes)
    }

    .debug_ranges : {
        *(.debug_ranges)
    }

    .debug_macro : {
        *(.debug_macro)
    }

    .debug_loc : {
        *(.debug_loc)
    }

    .debug_abbrev : {
        *(.debug_abbrev)
    }

    .debug_line_str : {
        *(.debug_line_str)
    }

    .debug_frame : {
        *(.debug_frame)
    }

    .debug_str_offsets : {
        *(.debug_str_offsets)
    }

    .debug_addr : {
        *(.debug_addr)
    }

    .debug_line_end : {
        *(.debug_line_end)
    }

    PROVIDE(kernel_start = 0xffffffff80000000 + 1M);

    PROVIDE(kernel_end = .);

    PROVIDE(heap_start = .);

    . += 4K;

    PROVIDE(heap_base = .);
}

