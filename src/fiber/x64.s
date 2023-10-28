.global xx_core_fiber_x64_start
xx_core_fiber_x64_start:
	mov rax, [rsp - 0x10]
	mov rdi, [rsp - 0x08]
	push qword ptr 0
	jmp rax

.global xx_core_fiber_x64_intercept
xx_core_fiber_x64_intercept:
	mov rax, [rsp - 0x18]
	mov rdi, [rsp - 0x10]
	sub rsp, 8
	jmp rax

.global xx_core_fiber_x64_switch
.align 16
xx_core_fiber_x64_switch:
	mov rax, [rsp]
	add rsp, 8

	mov [rdi + 0x00], rax
	mov [rdi + 0x08], rsp
	mov [rdi + 0x10], rbx
	mov [rdi + 0x18], rbp

	mov rax, [rsi + 0x00]
	mov rsp, [rsi + 0x08]
	mov rbx, [rsi + 0x10]
	mov rbp, [rsi + 0x18]

	jmp rax