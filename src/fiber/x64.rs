use std::{arch::global_asm, mem::zeroed};

use super::{Start, Intercept};

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

global_asm!(include_str!("x64.s"));

extern "C" {
	fn xx_core_fiber_start();
	fn xx_core_fiber_intercept();
	fn xx_core_fiber_switch(from: &mut Context, to: &mut Context);
}

impl Context{
	pub fn new() -> Context{
		unsafe { zeroed() }
	}

	pub(crate) fn set_stack(&mut self, stack: usize, len: usize){
		self.rsp = (stack + len) as u64;
	}

	pub(crate) fn set_start(&mut self, start: Start){
		let ptr = unsafe {
			&mut *(self.rsp as *mut () as *mut Start).offset(-1)
		};

		self.rip = xx_core_fiber_start as u64;

		*ptr = start;
	}

	pub(crate) fn set_intercept(&mut self, mut intercept: Intercept){
		let ptr = unsafe {
			&mut *(self.rsp as *mut () as *mut Intercept).offset(-1)
		};

		if intercept.ret == 0 {
			intercept.ret = self.rip as usize;
		}

		self.rip = xx_core_fiber_intercept as u64;

		*ptr = intercept;
	}
}

#[inline(always)]
pub unsafe fn switch(from: &mut Context, to: &mut Context){
	xx_core_fiber_switch(from, to);
}