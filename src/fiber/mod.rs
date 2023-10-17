use std::mem::ManuallyDrop;

use enumflags2::make_bitflags;

use super::{
	os::{
		mman::{map_memory, MemoryFlag, MemoryMap, MemoryProtection, MemoryType},
		resource::{get_limit, Resource}
	},
	sysdep::import_sysdeps
};
use crate::pointer::{ConstPtr, MutPtr};

import_sysdeps!();

#[allow(dead_code)]
pub struct Fiber {
	context: Context,

	stack: MemoryMap
}

/// Safety: the stack is not used before a fiber is started,
/// so we can safely write our start args there
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Start {
	pub(crate) start: usize,
	pub(crate) arg: *const ()
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
pub(crate) struct Intercept {
	pub intercept: usize,
	pub arg: *const (),
	pub ret: usize
}

extern "C" fn exit_fiber(arg: *const ()) {
	let mut fiber: MutPtr<ManuallyDrop<Fiber>> = ConstPtr::from(arg).cast();

	/* Safety: move worker off of its own stack then drop */
	unsafe {
		drop(ManuallyDrop::take(&mut fiber));
	};
}

impl Fiber {
	pub fn main() -> Self {
		Fiber { context: Context::new(), stack: MemoryMap::new() }
	}

	pub fn new(start: Start) -> Self {
		let stack = map_memory(
			0,
			get_limit(Resource::Stack).expect("Failed to get stack size") as usize,
			make_bitflags!(MemoryProtection::{Read | Write}).bits(),
			MemoryType::Private as u32 | make_bitflags!(MemoryFlag::{Anonymous | Stack}).bits(),
			None,
			0
		)
		.unwrap();

		let mut context = Context::new();

		context.set_stack(stack.addr, stack.length);
		context.set_start(start);

		Self { context, stack }
	}

	/// Switch from the fiber `self` to the new fiber `to`
	///
	/// Safety: `self` must be currently running
	#[inline(always)]
	pub unsafe fn switch(&mut self, to: &mut Fiber) {
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
}
