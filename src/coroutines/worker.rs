use super::executor::Executor;
use crate::{
	fiber::Fiber,
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

	pub fn new(executor: Handle<Executor>) -> Worker {
		Worker { executor, fiber: Fiber::new() }
	}

	#[inline(always)]
	pub fn executor(&mut self) -> Handle<Executor> {
		self.executor
	}

	#[inline(always)]
	pub(crate) unsafe fn inner(&mut self) -> &mut Fiber {
		&mut self.fiber
	}

	#[inline(always)]
	pub unsafe fn suspend(&mut self) {
		self.executor.clone().suspend(self.into());
	}

	#[inline(always)]
	pub unsafe fn resume(&mut self) {
		self.executor.clone().switch_to(self.into());
	}
}

impl Global for Worker {}
