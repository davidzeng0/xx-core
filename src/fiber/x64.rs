use super::*;

#[repr(C)]
pub(super) struct Context {
	rip: u64,
	rsp: u64,
	rbx: u64,
	rbp: u64
}

global_asm!(include_str!("x64.s"));

extern "C" {
	fn xx_core_fiber_x64_start();
	fn xx_core_fiber_x64_intercept();
	fn xx_core_fiber_x64_switch(from: &mut Context, to: &mut Context);
}

impl Context {
	pub fn new() -> Self {
		unsafe { zeroed() }
	}

	pub fn program_counter(&self) -> Ptr<()> {
		Ptr::from_int_addr(self.rip as usize)
	}

	pub unsafe fn set_stack(&mut self, stack: Ptr<()>, len: usize) {
		self.rsp = (stack.int_addr() + len) as u64;
	}

	pub unsafe fn set_start(&mut self, start: Start) {
		let stack = MutPtr::<Start>::from_int_addr(self.rsp as usize);

		stack.sub(1).write(start);

		self.rip = xx_core_fiber_x64_start as u64;
	}

	pub unsafe fn set_intercept(&mut self, intercept: Intercept) {
		let stack = MutPtr::<Intercept>::from_int_addr(self.rsp as usize);

		stack.sub(1).write(intercept);

		self.rip = xx_core_fiber_x64_intercept as u64;
	}
}

pub(super) unsafe fn switch(from: &mut Context, to: &mut Context) {
	unsafe {
		asm!(
			"call {}",
			sym xx_core_fiber_x64_switch,
			in("rdi") from,
			in("rsi") to,
			lateout("r12") _,
			lateout("r13") _,
			lateout("r14") _,
			lateout("r15") _,
			clobber_abi("C")
		)
	}
}
