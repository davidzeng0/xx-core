use std::mem::replace;

use super::*;

/// Per thread executor, responsible for running worker threads
pub struct Executor {
	pool: Ptr<Pool>,
	main: Worker,
	current: UnsafeCell<Ptr<Worker>>
}

impl Executor {
	pub fn new() -> Self {
		unsafe { Self::new_with_pool(Ptr::null()) }
	}

	pub unsafe fn new_with_pool(pool: Ptr<Pool>) -> Self {
		Self {
			pool,
			main: Worker::main(),

			/* current cannot be null, is assigned once pinned */
			current: UnsafeCell::new(Ptr::null())
		}
	}

	pub fn new_worker(&self, start: Start) -> Worker {
		if self.pool.is_null() {
			Worker::new(self.into(), start)
		} else {
			unsafe { Worker::from_fiber(self.into(), self.pool.new_fiber(start)) }
		}
	}

	/// Workers move themselves onto their own stack when
	/// they get started. We have to update our current reference
	/// when they get moved and pinned
	pub(super) unsafe fn worker_pinned(&self, worker: Ptr<Worker>) {
		*self.current.as_mut() = worker;
	}

	/// Suspend the `worker` and resume on the worker that resumed `worker`
	///
	/// Safety: the passed `worker` must be the current worker running
	pub(super) unsafe fn suspend(&self, worker: Ptr<Worker>) {
		let from = worker.source();

		*self.current.as_mut() = from;

		worker.fiber().switch(from.fiber());
	}

	/// Switch from whichever `current` worker is running to the new `worker`
	///
	/// Safety: the passed `worker` must not exist on the worker call stack
	/// Workers cannot resume each other recursively
	pub(super) unsafe fn resume(&self, worker: Ptr<Worker>) {
		let previous = replace(self.current.as_mut(), worker);

		worker.suspend_to(previous);
		previous.fiber().switch(worker.fiber());
	}

	/// Start a new worker
	///
	/// Safety: same as resume
	pub(super) unsafe fn start(&self, worker: Ptr<Worker>) {
		self.resume(worker);
	}

	/// Exit the worker and drop its stack
	///
	/// Safety: same as resume
	pub(super) unsafe fn exit(&self, worker: Worker) {
		let pool = self.pool;
		let from = worker.source();

		*self.current.as_mut() = from;

		if pool.is_null() {
			worker.into_inner().exit(from.fiber())
		} else {
			worker.into_inner().exit_to_pool(from.fiber(), pool);
		}
	}
}

unsafe impl Pin for Executor {
	unsafe fn pin(&mut self) {
		*self.current.as_mut() = Ptr::from(&self.main);
	}
}
