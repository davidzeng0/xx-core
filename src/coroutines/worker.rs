use super::executor::Executor;
use crate::{
	fiber::{Fiber, Start},
	task::env::{Global, Handle}
};

/// A worker thread capable of running async operations via fibers
pub struct Worker {
	executor: Handle<Executor>,
	from: Handle<Worker>,
	fiber: Fiber
}

impl Worker {
	pub fn main() -> Self {
		Self {
			executor: unsafe { Handle::new_null() },
			from: unsafe { Handle::new_null() },
			fiber: Fiber::main()
		}
	}

	pub fn new(executor: Handle<Executor>, start: Start) -> Self {
		Self {
			executor,
			from: unsafe { Handle::new_null() },
			fiber: Fiber::new(start)
		}
	}

	pub(crate) fn from(&self) -> Handle<Worker> {
		self.from
	}

	pub(crate) fn set_resume_to(&mut self, from: Handle<Worker>) {
		self.from = from;
	}

	pub(crate) fn inner(&mut self) -> &mut Fiber {
		&mut self.fiber
	}

	pub(crate) fn into_inner(self) -> Fiber {
		self.fiber
	}

	#[inline(always)]
	pub unsafe fn resume(&mut self) {
		self.executor.clone().resume(self.into());
	}

	#[inline(always)]
	pub unsafe fn suspend(&mut self) {
		self.executor.clone().suspend(self.into());
	}

	pub unsafe fn exit(self) {
		self.executor.clone().exit(self);
	}
}

impl Global for Worker {
	unsafe fn pinned(&mut self) {
		self.executor.clone().worker_pinned(self.into());
	}
}
