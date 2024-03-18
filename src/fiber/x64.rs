use super::*;

define_context! {
	pub(super) struct Context {
		rip: usize,
		rsp: usize,
		rbx: usize,
		rbp: usize
	}
}

global_asm!(include_str!("x64.s"));

extern "C" {
	fn xx_core_fiber_x64_start();
	fn xx_core_fiber_x64_intercept();
	fn xx_core_fiber_x64_switch(from: &mut Context, to: &mut Context);
}

impl Context {
	pub const fn program_counter(&self) -> Ptr<()> {
		Ptr::from_int_addr(self.rip)
	}

	pub unsafe fn set_stack(&mut self, stack: Ptr<()>, len: usize) {
		#[allow(clippy::arithmetic_side_effects)]
		(self.rsp = stack.int_addr() + len);
	}

	pub unsafe fn set_start(&mut self, start: Start) {
		let stack = MutPtr::<Start>::from_int_addr(self.rsp);

		/* Safety: guaranteed by caller */
		#[allow(clippy::arithmetic_side_effects)]
		unsafe {
			(stack - 1).write(start);
		}

		self.rip = xx_core_fiber_x64_start as usize;
	}

	pub unsafe fn set_intercept(&mut self, intercept: Intercept) {
		let stack = MutPtr::<Intercept>::from_int_addr(self.rsp);

		/* Safety: guaranteed by caller */
		#[allow(clippy::arithmetic_side_effects)]
		unsafe {
			(stack - 1).write(intercept);
		}

		self.rip = xx_core_fiber_x64_intercept as usize;
	}
}

pub(super) unsafe fn switch(from: &mut Context, to: &mut Context) {
	/* Safety: guaranteed by caller */
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
		);
	}
}
