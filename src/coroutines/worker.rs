use super::executor::Executor;
use crate::{
	fiber::{Fiber, Start},
	task::env::{Global, Handle}
};

/// A worker thread capable of running async operations via fibers
pub struct Worker {
	executor: Handle<Executor>,
	fiber: Fiber
}

impl Worker {
	pub fn main() -> Worker {
		Worker {
			executor: unsafe { Handle::new_empty() },
			fiber: Fiber::main()
		}
	}

	pub fn new(executor: Handle<Executor>, start: Start) -> Worker {
		Worker { executor, fiber: Fiber::new(start) }
	}

	#[inline(always)]
	pub fn executor(&mut self) -> Handle<Executor> {
		self.executor
	}

	#[inline(always)]
	pub(crate) unsafe fn inner(&mut self) -> &mut Fiber {
		&mut self.fiber
	}

	pub(crate) unsafe fn into_inner(self) -> Fiber {
		self.fiber
	}

	#[inline(always)]
	pub unsafe fn suspend(&mut self) {
		self.executor.clone().suspend(self.into());
	}

	#[inline(always)]
	pub unsafe fn resume(&mut self) {
		self.executor.clone().switch_to(self.into());
	}

	pub unsafe fn exit(self) {
		self.executor.clone().exit(self);
	}
}

impl Global for Worker {}
