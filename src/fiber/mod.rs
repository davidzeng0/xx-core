use std::mem::ManuallyDrop;

use enumflags2::make_bitflags;

use super::{
	os::{
		mman::{map_memory, MemoryFlag, MemoryMap, MemoryProtection, MemoryType},
		resource::{get_limit, Resource}
	},
	sysdep::import_sysdeps
};

import_sysdeps!();

#[allow(dead_code)]
pub struct Fiber {
	context: Context,

	stack: MemoryMap
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Start {
	pub start: extern "C" fn(*const ()),
	pub arg: *const ()
}

#[repr(C)]
pub(crate) struct Intercept {
	pub intercept: usize,
	pub arg: *const (),
	pub ret: usize
}

extern "C" fn exit_fiber(arg: *const ()) {
	let fiber = unsafe { &mut *(arg as *mut ManuallyDrop<Fiber>) };

	unsafe {
		ManuallyDrop::drop(fiber);
	}
}

impl Fiber {
	pub fn main() -> Fiber {
		Fiber { context: Context::new(), stack: MemoryMap::new() }
	}

	pub fn new(start: Start) -> Fiber {
		let stack = map_memory(
			0,
			get_limit(Resource::Stack).unwrap() as usize,
			make_bitflags!(MemoryProtection::{Read | Write}).bits(),
			MemoryType::Private as u32 | make_bitflags!(MemoryFlag::{Anonymous | Stack}).bits(),
			None,
			0
		)
		.unwrap();

		let mut context = Context::new();

		context.set_stack(stack.addr, stack.length);
		context.set_start(start);

		Fiber { context, stack }
	}

	#[inline(always)]
	pub unsafe fn switch(&mut self, to: &mut Fiber) {
		switch(&mut self.context, &mut to.context);
	}

	pub unsafe fn exit(self, to: &mut Fiber) {
		let mut fiber = ManuallyDrop::new(self);

		to.context.set_intercept(Intercept {
			intercept: exit_fiber as usize,
			arg: &mut fiber as *mut _ as *const (),
			ret: 0
		});

		fiber.switch(to);
	}
}
