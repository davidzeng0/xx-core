use std::mem::ManuallyDrop;

use enumflags2::make_bitflags;

use super::{
	os::{mman::*, resource::*},
	sysdep::import_sysdeps
};
use crate::pointer::*;

import_sysdeps!();

pub mod pool;
pub use self::pool::Pool;

#[allow(dead_code)]
pub struct Fiber {
	context: Context,

	stack: MemoryMap<'static>
}

/// Safety: the stack is not used before a fiber is started,
/// so we can safely write our start args there
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Start {
	start: usize,
	arg: *const ()
}

impl Start {
	pub fn new(start: extern "C" fn(*const ()), arg: *const ()) -> Self {
		Self { start: start as usize, arg }
	}

	pub fn set_start(&mut self, start: extern "C" fn(*const ())) {
		self.start = start as usize;
	}

	pub fn set_arg(&mut self, arg: *const ()) {
		self.arg = arg;
	}
}

/// Safety: when fiber A suspends to B and
/// B exits to A, A gets intercepted
///
/// A called a non-inline switch to B, meaning any lower addresss stack space in
/// A is not in use
///
/// and A's intercept can be written on the stack
#[repr(C)]
#[derive(Clone, Copy)]
struct Intercept {
	intercept: usize,
	arg: *const (),
	ret: usize
}

extern "C" fn exit_fiber(arg: *const ()) {
	let mut fiber: MutPtr<ManuallyDrop<Fiber>> = ConstPtr::from(arg).cast();

	/* Safety: move worker off of its own stack then drop */
	unsafe {
		drop(ManuallyDrop::take(&mut fiber));
	};
}

extern "C" fn exit_fiber_to_pool(arg: *const ()) {
	let mut arg: MutPtr<(ManuallyDrop<Fiber>, MutPtr<Pool>)> = ConstPtr::from(arg).cast();
	let fiber = unsafe { ManuallyDrop::take(&mut arg.0) };

	arg.1.exit_fiber(fiber);
}

impl Fiber {
	pub fn main() -> Self {
		Fiber { context: Context::new(), stack: MemoryMap::new() }
	}

	pub fn new() -> Self {
		Self {
			/* fiber context. stores to-be-preserved registers,
			 * including any that cannot be corrupted by inline asm
			 */
			context: Context::new(),
			stack: map_memory(
				0,
				get_limit(Resource::Stack).expect("Failed to get stack size") as usize,
				make_bitflags!(MemoryProtection::{Read | Write}).bits(),
				MemoryType::Private as u32 | make_bitflags!(MemoryFlag::{Anonymous | Stack}).bits(),
				None,
				0
			)
			.unwrap()
		}
	}

	pub fn new_with_start(start: Start) -> Self {
		let mut this = Self::new();

		unsafe {
			this.set_start(start);
		}

		this
	}

	pub unsafe fn set_start(&mut self, start: Start) {
		/* set the stack back to the beginning. unuse all the stack that the previous
		 * worker used */
		self.context
			.set_stack(self.stack.addr(), self.stack.length());
		self.context.set_start(start);
	}

	/// Switch from the fiber `self` to the new fiber `to`
	///
	/// Safety: `self` must be currently running
	#[inline(always)]
	pub unsafe fn switch(&mut self, to: &mut Fiber) {
		/* note for arch specific implementation:
		 * all registers must be declared clobbered
		 *
		 * it's faster to let the compiler preserve the
		 * registers it knows it uses rather than
		 * having the functions written in assembly
		 * store them for us
		 */
		switch(&mut self.context, &mut to.context);
	}

	/// Same as switch, except drops the `self` fiber
	///
	/// Worker is unpinned and consumed
	pub unsafe fn exit(self, to: &mut Fiber) {
		let mut fiber = ManuallyDrop::new(self);

		to.context.set_intercept(Intercept {
			intercept: exit_fiber as usize,
			arg: MutPtr::from(&mut fiber).as_raw_ptr(),
			ret: 0
		});

		fiber.switch(to);
	}

	pub unsafe fn exit_to_pool(self, to: &mut Fiber, pool: MutPtr<Pool>) {
		let mut arg = (ManuallyDrop::new(self), pool);

		to.context.set_intercept(Intercept {
			intercept: exit_fiber_to_pool as usize,
			arg: MutPtr::from(&mut arg).as_raw_ptr(),
			ret: 0
		});

		arg.0.switch(to);
	}
}
