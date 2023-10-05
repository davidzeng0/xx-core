use std::{arch::global_asm, mem::zeroed};

#[repr(C)]
pub struct Context{
	r12: u64,
	r13: u64,
	r14: u64,
	r15: u64,
	rip: u64,
	rsp: u64,
	rbx: u64,
	rbp: u64,
}

impl Context{
	pub fn new() -> Context{
		unsafe { zeroed() }
	}

	pub fn set_stack(&mut self, stack: usize, len: usize){
		self.rsp = (stack + len) as u64;
	}
}

global_asm!{
".global xx_core_fiber_start",
"xx_core_fiber_start:",
	"mov r8, [rsp]",
	"add rsp, 8",

	"mov [rdi + 0x00], r12",
	"mov [rdi + 0x08], r13",
	"mov [rdi + 0x10], r14",
	"mov [rdi + 0x18], r15",
	"mov [rdi + 0x20], r8",
	"mov [rdi + 0x28], rsp",
	"mov [rdi + 0x30], rbx",
	"mov [rdi + 0x38], rbp",

	"xor ebp, ebp",
	"mov rsp, [rsi + 0x28]",
	"mov rdi, rcx",
	"push qword ptr 0",
	"jmp rdx"
}

global_asm!{
".global xx_core_fiber_switch",
"xx_core_fiber_switch:",
	"mov rdx, [rsp]",
	"add rsp, 8",

	"mov [rdi + 0x00], r12",
	"mov [rdi + 0x08], r13",
	"mov [rdi + 0x10], r14",
	"mov [rdi + 0x18], r15",
	"mov [rdi + 0x20], rdx",
	"mov [rdi + 0x28], rsp",
	"mov [rdi + 0x30], rbx",
	"mov [rdi + 0x38], rbp",

	"mov r12, [rsi + 0x00]",
	"mov r13, [rsi + 0x08]",
	"mov r14, [rsi + 0x10]",
	"mov r15, [rsi + 0x18]",
	"mov rax, [rsi + 0x20]",
	"mov rsp, [rsi + 0x28]",
	"mov rbx, [rsi + 0x30]",
	"mov rbp, [rsi + 0x38]",

	"jmp rax"
}

extern "C" {
	fn xx_core_fiber_start(from: &mut Context, to: &mut Context, f: extern fn(*const ()), arg: *const ());
	fn xx_core_fiber_switch(from: &mut Context, to: &mut Context);
}

#[inline(always)]
pub unsafe fn start(from: &mut Context, to: &mut Context, f: extern fn(*const ()), arg: *const ()){
	xx_core_fiber_start(from, to, f, arg);
}

#[inline(always)]
pub unsafe fn switch(from: &mut Context, to: &mut Context){
	xx_core_fiber_switch(from, to);
}