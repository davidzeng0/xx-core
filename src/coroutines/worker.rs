use super::executor::Executor;
use crate::{fiber::*, task::*};

/// A worker thread capable of running async operations via fibers
pub struct Worker {
	executor: Handle<Executor>,
	from: Handle<Worker>,
	fiber: Fiber
}

impl Worker {
	pub fn main() -> Self {
		Self::from_fiber(unsafe { Handle::new_null() }, Fiber::main())
	}

	pub fn new(executor: Handle<Executor>, start: Start) -> Self {
		Self::from_fiber(executor, Fiber::new(start))
	}

	pub fn from_fiber(executor: Handle<Executor>, fiber: Fiber) -> Self {
		Self {
			executor,
			from: unsafe { Handle::new_null() },
			fiber
		}
	}

	/// The worker that `self` will resume to when suspending
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
