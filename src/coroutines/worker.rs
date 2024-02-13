use std::cell::Cell;

use super::*;

/// A worker thread capable of running async operations via fibers
pub struct Worker {
	executor: Ptr<Executor>,
	caller: Cell<Ptr<Worker>>,
	fiber: UnsafeCell<Fiber>
}

impl Worker {
	/// The worker for the main thread, which does not need
	/// an extra stack allocation, because it's allocated for us
	pub fn main() -> Self {
		/* Safety: this is the main fiber */
		unsafe { Self::from_fiber(Ptr::null(), Fiber::main()) }
	}

	/// Creates a new worker with the starting point `start`
	///
	/// Safety: executor must be valid for the duration of the worker
	pub unsafe fn new(executor: Ptr<Executor>, start: Start) -> Self {
		/* Safety: contract is upheld by caller */
		unsafe { Self::from_fiber(executor, Fiber::new_with_start(start)) }
	}

	/// Safety: user must call Fiber::set_start and executor must be valid for
	/// the duration of the worker, unless its a main fiber
	pub unsafe fn from_fiber(executor: Ptr<Executor>, fiber: Fiber) -> Self {
		Self {
			executor,

			/* from is initialized later */
			caller: Cell::new(Ptr::null()),
			fiber: UnsafeCell::new(fiber)
		}
	}

	/// The worker that `self` will resume to when suspending
	pub(super) fn caller(&self) -> Ptr<Worker> {
		self.caller.get()
	}

	pub(super) fn suspend_to(&self, to: Ptr<Worker>) {
		self.caller.set(to);
	}

	pub(super) unsafe fn fiber(&self) -> &mut Fiber {
		self.fiber.as_mut()
	}

	pub(super) fn into_inner(self) -> Fiber {
		self.fiber.into_inner()
	}

	pub(super) unsafe fn resume(&self) {
		self.executor.as_ref().resume(self.into());
	}

	pub(super) unsafe fn suspend(&self) {
		self.executor.as_ref().suspend(self.into());
	}

	pub(super) unsafe fn exit(self) {
		self.executor.as_ref().exit(self);
	}
}

unsafe impl Pin for Worker {
	unsafe fn pin(&mut self) {
		self.executor.as_ref().worker_pinned(self.into());
	}
}
