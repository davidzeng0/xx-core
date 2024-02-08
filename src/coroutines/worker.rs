use super::*;

/// A worker thread capable of running async operations via fibers
pub struct Worker {
	executor: Ptr<Executor>,
	from: UnsafeCell<Ptr<Worker>>,
	fiber: UnsafeCell<Fiber>
}

impl Worker {
	/// The worker for the main thread, which does not need
	/// an extra stack allocation, because it's allocated for us
	pub fn main() -> Self {
		unsafe {
			/* main worker does not need an executor */
			Self::from_fiber(Ptr::null(), Fiber::main())
		}
	}

	/// Creates a new worker with the starting point `start`
	pub fn new(executor: Ptr<Executor>, start: Start) -> Self {
		unsafe { Self::from_fiber(executor, Fiber::new_with_start(start)) }
	}

	/// Safety: user must call Fiber::set_start
	pub unsafe fn from_fiber(executor: Ptr<Executor>, fiber: Fiber) -> Self {
		Self {
			executor,

			/* from is initialized later */
			from: UnsafeCell::new(Ptr::null()),
			fiber: UnsafeCell::new(fiber)
		}
	}

	/// The worker that `self` will resume to when suspending
	pub(super) fn source(&self) -> Ptr<Worker> {
		*self.from.as_ref()
	}

	pub(super) fn suspend_to(&self, from: Ptr<Worker>) {
		*self.from.as_mut() = from;
	}

	pub(super) fn fiber(&self) -> &mut Fiber {
		self.fiber.as_mut()
	}

	pub(super) fn into_inner(self) -> Fiber {
		self.fiber.into_inner()
	}

	pub(super) unsafe fn resume(&self) {
		self.executor.clone().resume(self.into());
	}

	pub(super) unsafe fn suspend(&self) {
		self.executor.clone().suspend(self.into());
	}

	pub(super) unsafe fn exit(self) {
		self.executor.clone().exit(self);
	}
}

unsafe impl Pin for Worker {
	unsafe fn pin(&mut self) {
		self.executor.clone().worker_pinned(self.into());
	}
}
