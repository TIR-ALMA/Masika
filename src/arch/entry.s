.global _start
.code64

_start:
    mov $stack_top, %rsp
    call rust_main
    hlt

.align 16
stack_bottom:
    .skip 8192
stack_top:

.globl syscall_handler
syscall_handler:
    pushq %rax
    pushq %rcx
    pushq %rdx
    pushq %rsi
    pushq %rdi
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    sub $160, %rsp
    fxsave (%rsp)
    mov %rsp, %rdi
    call handle_syscall
    fxrstor (%rsp)
    add $160, %rsp
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rdi
    popq %rsi
    popq %rdx
    popq %rcx
    popq %rax
    sysretq

handle_syscall:
    pushq %rbp
    movq %rsp, %rbp
    pushq %rbx
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    movq %rdi, %r15
    movq %rsi, %r14
    movq %rdx, %r13
    movq %rcx, %r12
    movq %r8, %rbx
    movq %r9, %rbp
    call syscall_dispatch
    movq %rax, (%r15)
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbx
    popq %rbp
    ret

trap_frame_save:
    pushq %rax
    pushq %rbx
    pushq %rcx
    pushq %rdx
    pushq %rsi
    pushq %rdi
    pushq %rbp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    sub $160, %rsp
    fxsave (%rsp)
    ret

trap_frame_restore:
    fxrstor (%rsp)
    add $160, %rsp
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rbp
    popq %rdi
    popq %rsi
    popq %rdx
    popq %rcx
    popq %rbx
    popq %rax
    iretq

.globl context_switch
context_switch:
    pushq %rax
    pushq %rbx
    pushq %rcx
    pushq %rdx
    pushq %rsi
    pushq %rdi
    pushq %rbp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    sub $160, %rsp
    fxsave (%rsp)
    movq %rsp, (%rdi)
    movq (%rsi), %rsp
    fxrstor (%rsp)
    add $160, %rsp
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rbp
    popq %rdi
    popq %rsi
    popq %rdx
    popq %rcx
    popq %rbx
    popq %rax
    ret

.globl interrupt_common_stub
interrupt_common_stub:
    pushq %rax
    pushq %rbx
    pushq %rcx
    pushq %rdx
    pushq %rsi
    pushq %rdi
    pushq %rbp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    sub $160, %rsp
    fxsave (%rsp)
    call interrupt_handler
    fxrstor (%rsp)
    add $160, %rsp
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rbp
    popq %rdi
    popq %rsi
    popq %rdx
    popq %rcx
    popq %rbx
    popq %rax
    iretq

.globl save_fpu_state
save_fpu_state:
    sub $512, %rsp
    fxsave (%rsp)
    movq %rsp, (%rdi)
    ret

.globl restore_fpu_state
restore_fpu_state:
    movq (%rdi), %rsp
    fxrstor (%rsp)
    add $512, %rsp
    ret

.globl enable_interrupts
enable_interrupts:
    sti
    ret

.globl disable_interrupts
disable_interrupts:
    cli
    ret

.globl get_rflags
get_rflags:
    pushfq
    popq %rax
    ret

.globl set_rflags
set_rflags:
    pushq %rdi
    popfq
    ret

.globl read_cr2
read_cr2:
    movq %cr2, %rax
    ret

.globl read_cr3
read_cr3:
    movq %cr3, %rax
    ret

.globl write_cr3
write_cr3:
    movq %rdi, %cr3
    ret

.globl invalidate_page
invalidate_page:
    invlpg (%rdi)
    ret

.globl read_msr
read_msr:
    rdmsr
    shl $32, %rdx
    or %rdx, %rax
    ret

.globl write_msr
write_msr:
    mov %rdi, %rcx
    mov %rsi, %rax
    shr $32, %rsi
    mov %rsi, %rdx
    wrmsr
    ret

.globl pause_cpu
pause_cpu:
    pause
    ret

.globl halt_cpu
halt_cpu:
    hlt
    ret

.globl cpuid_leaf
cpuid_leaf:
    mov %rdi, %rax
    xor %rcx, %rcx
    cpuid
    mov %eax, (%rsi)
    mov %ebx, 4(%rsi)
    mov %ecx, 8(%rsi)
    mov %edx, 12(%rsi)
    ret

.globl lfence_op
lfence_op:
    lfence
    ret

.globl mfence_op
mfence_op:
    mfence
    ret

.globl sfence_op
sfence_op:
    sfence
    ret

.globl rdtsc_op
rdtsc_op:
    rdtsc
    shl $32, %rdx
    or %rdx, %rax
    ret

.globl rdtscp_op
rdtscp_op:
    rdtscp
    shl $32, %rdx
    or %rdx, %rax
    ret

.globl load_gdt
load_gdt:
    lgdt (%rdi)
    ret

.globl load_idt
load_idt:
    lidt (%rdi)
    ret

.globl load_tr
load_tr:
    ltr %di
    ret

.globl load_ldt
load_ldt:
    lldt %di
    ret

.globl tlb_flush_all
tlb_flush_all:
    mov %cr3, %rax
    mov %rax, %cr3
    ret

.globl read_fs_base
read_fs_base:
    rdfsbase %rax
    ret

.globl write_fs_base
write_fs_base:
    wrfsbase %rdi
    ret

.globl read_gs_base
read_gs_base:
    rdgsbase %rax
    ret

.globl write_gs_base
write_gs_base:
    wrgsbase %rdi
    ret

.globl swapgs_op
swapgs_op:
    swapgs
    ret

.globl xsave_area
xsave_area:
    xsave (%rsi)
    ret

.globl xrstor_area
xrstor_area:
    xrstor (%rsi)
    ret

.globl fxsave_area
fxsave_area:
    fxsave (%rdi)
    ret

.globl fxrstor_area
fxrstor_area:
    fxrstor (%rdi)
    ret

.globl clts_op
clts_op:
    clts
    ret

.globl stts_op
stts_op:
    mov %cr0, %rax
    or $8, %rax
    mov %rax, %cr0
    ret

.globl cr0_read
cr0_read:
    mov %cr0, %rax
    ret

.globl cr0_write
cr0_write:
    mov %rdi, %cr0
    ret

.globl cr4_read
cr4_read:
    mov %cr4, %rax
    ret

.globl cr4_write
cr4_write:
    mov %rdi, %cr4
    ret

.globl read_dr0
read_dr0:
    mov %dr0, %rax
    ret

.globl read_dr1
read_dr1:
    mov %dr1, %rax
    ret

.globl read_dr2
read_dr2:
    mov %dr2, %rax
    ret

.globl read_dr3
read_dr3:
    mov %dr3, %rax
    ret

.globl read_dr6
read_dr6:
    mov %dr6, %rax
    ret

.globl read_dr7
read_dr7:
    mov %dr7, %rax
    ret

.globl write_dr0
write_dr0:
    mov %rdi, %dr0
    ret

.globl write_dr1
write_dr1:
    mov %rdi, %dr1
    ret

.globl write_dr2
write_dr2:
    mov %rdi, %dr2
    ret

.globl write_dr3
write_dr3:
    mov %rdi, %dr3
    ret

.globl write_dr6
write_dr6:
    mov %rdi, %dr6
    ret

.globl write_dr7
write_dr7:
    mov %rdi, %dr7
    ret

.globl set_kernel_stack
set_kernel_stack:
    mov %rdi, %rsp
    ret

.globl get_kernel_stack
get_kernel_stack:
    mov %rsp, %rax
    ret

.globl switch_to_user
switch_to_user:
    mov %rdi, %rsp
    pushq $0x23
    pushq %rsi
    pushfq
    popq %rax
    and $0xFFFFFFFFFFFFFFFE, %rax
    pushq %rax
    pushq $0x1B
    pushq %rdx
    iretq

.globl switch_to_kernel
switch_to_kernel:
    mov %rdi, %rsp
    ret

.globl user_copy_safe
user_copy_safe:
    pushq %rbp
    movq %rsp, %rbp
    pushq %rbx
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    movq %rdi, %r15
    movq %rsi, %r14
    movq %rdx, %r13
    cld
    cmp $0, %r13
    je .L_copy_done
.L_copy_loop:
    movb (%r14), %bl
    movb %bl, (%r15)
    incq %r15
    incq %r14
    decq %r13
    jnz .L_copy_loop
.L_copy_done:
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbx
    popq %rbp
    ret

.globl memset_safe
memset_safe:
    pushq %rbp
    movq %rsp, %rbp
    pushq %rbx
    pushq %r12
    pushq %r13
    movq %rdi, %r12
    movq %rsi, %rbx
    movq %rdx, %r13
    cmp $0, %r13
    je .L_memset_done
.L_memset_loop:
    movb %bl, (%r12)
    incq %r12
    decq %r13
    jnz .L_memset_loop
.L_memset_done:
    popq %r13
    popq %r12
    popq %rbx
    popq %rbp
    ret

.globl strlen_safe
strlen_safe:
    pushq %rbp
    movq %rsp, %rbp
    movq %rdi, %rax
.L_strlen_loop:
    cmpb $0, (%rax)
    je .L_strlen_end
    incq %rax
    jmp .L_strlen_loop
.L_strlen_end:
    subq %rdi, %rax
    popq %rbp
    ret

.globl strcmp_safe
strcmp_safe:
    pushq %rbp
    movq %rsp, %rbp
    pushq %rbx
    pushq %r12
    pushq %r13
    movq %rdi, %r12
    movq %rsi, %r13
.L_strcmp_loop:
    movb (%r12), %bl
    cmpb %bl, (%r13)
    jne .L_strcmp_diff
    cmpb $0, %bl
    je .L_strcmp_equal
    incq %r12
    incq %r13
    jmp .L_strcmp_loop
.L_strcmp_equal:
    movq $0, %rax
    jmp .L_strcmp_exit
.L_strcmp_diff:
    movzbq %bl, %rax
    movzbq (%r13), %r12
    subq %r12, %rax
.L_strcmp_exit:
    popq %r13
    popq %r12
    popq %rbx
    popq %rbp
    ret

