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

impl Fiber {
	pub fn main() -> Fiber {
		Fiber { context: Context::new(), stack: MemoryMap::new() }
	}

	pub fn new() -> Fiber {
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

		Fiber { context, stack }
	}

	#[inline(always)]
	pub unsafe fn start(
		&mut self, routine: &mut Fiber, f: extern "C" fn(*const ()), arg: *const ()
	) {
		start(&mut self.context, &mut routine.context, f, arg);
	}

	#[inline(always)]
	pub unsafe fn resume(&mut self, to: &mut Fiber) {
		switch(&mut self.context, &mut to.context);
	}
}
