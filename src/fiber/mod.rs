use std::{
	arch::{asm, global_asm},
	mem::{zeroed, ManuallyDrop}
};

use enumflags2::make_bitflags;

use super::{
	macros::import_sysdeps,
	os::{mman::*, resource::*},
	pointer::*
};

import_sysdeps!();

mod pool;
pub use pool::*;

/// Safety: the stack is not used before a fiber is started,
/// so we can safely write our start args there
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Start {
	start: unsafe fn(Ptr<()>),
	arg: Ptr<()>
}

impl Start {
	pub fn new(start: unsafe fn(Ptr<()>), arg: Ptr<()>) -> Self {
		Self { start, arg }
	}

	pub fn set_start(&mut self, start: unsafe fn(Ptr<()>)) {
		self.start = start;
	}

	pub fn set_arg(&mut self, arg: Ptr<()>) {
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
	intercept: unsafe fn(Ptr<()>),
	arg: Ptr<()>,
	ret: Ptr<()>
}

unsafe fn exit_fiber(arg: Ptr<()>) {
	let fiber = arg.cast::<ManuallyDrop<Fiber>>().cast_mut().as_mut();

	/* Safety: move worker off of its own stack then drop,
	 * in case the fiber accesses its own fields after dropping
	 * the stack, which for now it doesn't, unless you're exiting
	 * the fiber to a pool
	 */
	let fiber = unsafe { ManuallyDrop::take(fiber) };

	drop(fiber);
}

unsafe fn exit_fiber_to_pool(arg: Ptr<()>) {
	let arg = arg
		.cast::<(ManuallyDrop<Fiber>, MutPtr<Pool>)>()
		.cast_mut()
		.as_mut();
	/* Safety: ownership of the fiber is passed to us */
	let mut fiber = unsafe { ManuallyDrop::take(&mut arg.0) };

	fiber.clear_stack();
	arg.1.as_ref().exit_fiber(fiber);
}

pub struct Fiber {
	context: Context,
	stack: MemoryMap<'static>
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
			stack: MemoryMap::map(
				None,
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

		/* Safety: the fiber was never started */
		unsafe { this.set_start(start) };

		this
	}

	/// Set the entry point of the fiber
	///
	/// Safety: fiber is exited, or wasn't started
	pub unsafe fn set_start(&mut self, start: Start) {
		/* Safety: contract upheld by caller. the fiber isn't in running, so we can
		 * reset its state */
		unsafe {
			/* set the stack back to the beginning. unuse all the stack that the previous
			 * worker used */
			self.context
				.set_stack(self.stack.addr().cast_const(), self.stack.length());
			self.context.set_start(start);
		}
	}

	/// Switch from the fiber `self` to the new fiber `to`
	///
	/// Safety: `self` must be currently running
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

	pub unsafe fn clear_stack(&mut self) {
		let _ = self.stack.advise(MemoryAdvice::Free as u32);
	}

	/// Same as switch, except drops the `self` fiber
	///
	/// Worker is unpinned and consumed
	///
	/// Safety: same as switch
	pub unsafe fn exit(self, to: &mut Fiber) {
		let mut fiber = ManuallyDrop::new(self);

		/* Safety: contract upheld by caller */
		unsafe {
			to.context.set_intercept(Intercept {
				intercept: exit_fiber,
				arg: MutPtr::from(&mut fiber).as_unit().into(),
				ret: to.context.program_counter()
			});

			fiber.switch(to);
		}
	}

	/// Exits the fiber, storing the stack into a pool
	/// to be reused when a new fiber is spawned
	///
	/// Safety: same as above
	pub unsafe fn exit_to_pool(self, to: &mut Fiber, pool: Ptr<Pool>) {
		let mut arg = (ManuallyDrop::new(self), pool);

		/* Safety: contract upheld by caller */
		unsafe {
			to.context.set_intercept(Intercept {
				intercept: exit_fiber_to_pool,
				arg: MutPtr::from(&mut arg).as_unit().into(),
				ret: to.context.program_counter()
			});

			arg.0.switch(to);
		}
	}
}
