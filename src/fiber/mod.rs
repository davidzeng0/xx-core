#![allow(unreachable_pub, clippy::multiple_unsafe_ops_per_block)]

use std::{
	arch::global_asm,
	mem::{zeroed, ManuallyDrop}
};

use super::{
	macros::import_sysdeps,
	os::{mman::*, resource::*},
	pointer::*
};
use crate::{assert_unsafe_precondition, macros::panic_nounwind, opt::hint::unreachable_unchecked};

import_sysdeps!();

macro_rules! define_context {
	(
		pub struct $name: ident
		$($rest: tt)*
	) => {
		#[repr(C)]
		pub struct $name $($rest)*

		impl Default for $name {
			fn default() -> Self {
				/* Safety: repr(C) */
				unsafe { zeroed() }
			}
		}
	}
}

use define_context;

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
	/// # Safety
	/// See `set_start`
	pub unsafe fn new(start: unsafe fn(Ptr<()>), arg: Ptr<()>) -> Self {
		Self { start, arg }
	}

	/// # Safety
	/// `start` must never panic. must exit the worker before returning.
	/// care must be taken to drop any values before a call to exit
	///
	/// `start`'s safety contract is
	/// - called once when worker is started
	pub unsafe fn set_start(&mut self, start: unsafe fn(Ptr<()>)) {
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
	/* Safety: guaranteed by caller */
	let fiber = unsafe { arg.cast::<ManuallyDrop<Fiber>>().cast_mut().as_mut() };

	/* Safety: move worker off of its own stack then drop,
	 * in case the fiber accesses its own fields after dropping
	 * the stack, which for now it doesn't, unless you're exiting
	 * the fiber to a pool
	 */
	drop(unsafe { ManuallyDrop::take(fiber) });
}

unsafe fn exit_fiber_to_pool(arg: Ptr<()>) {
	/* Safety: guaranteed by caller */
	let arg = unsafe {
		arg.cast::<(ManuallyDrop<Fiber>, Ptr<Pool>)>()
			.cast_mut()
			.as_mut()
	};

	/* Safety: ownership of the fiber is passed to us */
	let mut fiber = unsafe { ManuallyDrop::take(&mut arg.0) };

	/* Safety: guaranteed by caller */
	unsafe {
		fiber.clear_stack();

		ptr!(arg.1=>exit_fiber(fiber));
	}
}

#[repr(C)]
pub struct Fiber {
	context: Context,
	stack: Map<'static>
}

impl Fiber {
	#[must_use]
	pub fn main() -> Self {
		Self { context: Context::default(), stack: Map::new() }
	}

	#[allow(clippy::new_without_default, clippy::expect_used, clippy::unwrap_used)]
	#[must_use]
	/// # Panics
	/// If the stack allocation fails
	pub fn new() -> Self {
		let stack_size = get_limit(Resource::Stack)
			.expect("Failed to get stack size")
			.try_into()
			.unwrap();

		assert!(stack_size > 0);

		let stack = Builder::new(Type::Private, stack_size)
			.protect(Protection::Read | Protection::Write)
			.flag(Flag::Anonymous | Flag::Stack)
			.map()
			.expect("Failed to allocate stack for fiber");

		Self {
			/* fiber context. stores to-be-preserved registers,
			 * including any that cannot be corrupted by inline asm
			 */
			context: Context::default(),
			stack
		}
	}

	#[must_use]
	pub fn new_with_start(start: Start) -> Self {
		let mut this = Self::new();

		/* Safety: the fiber was never started */
		unsafe { this.set_start(start) };

		this
	}

	/// Set the entry point of the fiber
	///
	/// # Safety
	/// fiber must not be running
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
	/// # Safety
	/// `self` must be currently running
	pub unsafe fn switch(this: MutPtr<Self>, to: MutPtr<Self>) {
		/* Safety: guaranteed by caller */
		unsafe { assert_unsafe_precondition!(!this.is_null() && !to.is_null()) };

		/* note for arch specific implementation:
		 * all registers must be declared clobbered
		 *
		 * it's faster to let the compiler preserve the
		 * registers it knows it uses rather than
		 * having the functions written in assembly
		 * store them for us
		 */

		/* Safety: guaranteed by caller */
		unsafe { switch(ptr!(&mut this=>context), ptr!(&mut to=>context)) };
	}

	/// # Safety
	/// fiber must not be running
	pub unsafe fn clear_stack(&mut self) {
		/* Safety: fiber isn't running */
		let _ = unsafe { self.stack.advise(Advice::Free) };
	}

	/// Same as switch, except drops the `self` fiber
	///
	/// Worker is unpinned and consumed
	///
	/// # Safety
	/// same as switch
	pub unsafe fn exit(self, to: MutPtr<Self>) -> ! {
		/* Safety: guaranteed by caller */
		unsafe { assert_unsafe_precondition!(!to.is_null()) };

		let mut fiber = ManuallyDrop::new(self);
		let ptr = ptr!(&mut fiber);

		/* Safety: contract upheld by caller */
		unsafe {
			let context = &mut ptr!(to=>context);

			context.set_intercept(Intercept {
				intercept: exit_fiber,
				arg: ptr.cast_const().cast(),
				ret: context.program_counter()
			});

			Self::switch(ptr.cast(), to);

			unreachable_unchecked();
		}
	}

	/// Exits the fiber, storing the stack into a pool
	/// to be reused when a new fiber is spawned
	///
	/// # Safety
	/// same as above
	pub unsafe fn exit_to_pool(self, to: MutPtr<Self>, pool: Ptr<Pool>) -> ! {
		/* Safety: guaranteed by caller */
		unsafe { assert_unsafe_precondition!(!to.is_null()) };

		let mut arg = (ManuallyDrop::new(self), pool);
		let ptr = ptr!(&mut arg);

		/* Safety: contract upheld by caller */
		unsafe {
			let context = &mut ptr!(to=>context);

			context.set_intercept(Intercept {
				intercept: exit_fiber_to_pool,
				arg: ptr.cast_const().cast(),
				ret: context.program_counter()
			});

			Self::switch(ptr!(&mut ptr=>0).cast(), to);

			unreachable_unchecked();
		}
	}
}

/* Safety: the stack is owned by the fiber */
unsafe impl Send for Fiber {}
