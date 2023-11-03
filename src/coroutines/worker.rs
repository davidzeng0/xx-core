use super::*;
use crate::fiber::*;

/// A worker thread capable of running async operations via fibers
pub struct Worker {
	executor: Handle<Executor>,
	from: Handle<Worker>,
	fiber: Fiber
}

impl Worker {
	/// The worker for the main thread, which does not need
	/// an extra stack allocation, because it's allocated for us
	pub fn main() -> Self {
		unsafe {
			/* main worker does not need an executor */
			Self::from_fiber(Handle::null(), Fiber::main())
		}
	}

	/// Creates a new worker with the starting point `start`
	pub fn new(executor: Handle<Executor>, start: Start) -> Self {
		unsafe { Self::from_fiber(executor, Fiber::new_with_start(start)) }
	}

	/// Safety: user must call Fiber::set_start
	pub unsafe fn from_fiber(executor: Handle<Executor>, fiber: Fiber) -> Self {
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
	pub(super) unsafe fn resume(&mut self) {
		self.executor.clone().resume(self.into());
	}

	#[inline(always)]
	pub(super) unsafe fn suspend(&mut self) {
		self.executor.clone().suspend(self.into());
	}

	pub(super) unsafe fn exit(self) {
		self.executor.clone().exit(self);
	}
}

impl Global for Worker {
	unsafe fn pinned(&mut self) {
		self.executor.clone().worker_pinned(self.into());
	}
}
