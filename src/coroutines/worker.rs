use super::*;
use crate::fiber::*;

/// A worker thread capable of running async operations via fibers
pub struct Worker {
	executor: Handle<Executor>,
	from: Handle<Worker>,
	fiber: Fiber
}

impl Worker {
	pub fn main() -> Self {
		Self::from_fiber(unsafe { Handle::null() }, Fiber::main())
	}

	pub fn new(executor: Handle<Executor>, start: Start) -> Self {
		Self::from_fiber(executor, Fiber::new_with_start(start))
	}

	pub fn from_fiber(executor: Handle<Executor>, fiber: Fiber) -> Self {
		Self {
			executor,

			/* from is initialized later */
			from: unsafe { Handle::null() },
			fiber
		}
	}

	/// The worker that `self` will resume to when suspending
	pub(super) fn from(&self) -> Handle<Worker> {
		self.from
	}

	pub(super) fn set_resume_to(&mut self, from: Handle<Worker>) {
		self.from = from;
	}

	pub(super) fn inner(&mut self) -> &mut Fiber {
		&mut self.fiber
	}

	pub(super) fn into_inner(self) -> Fiber {
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
