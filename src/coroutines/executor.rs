use std::cell::Cell;

use super::*;

/// Per thread executor, responsible for running worker threads
pub struct Executor {
	pool: Ptr<Pool>,
	main: Worker,
	current: Cell<Ptr<Worker>>
}

impl Executor {
	pub fn new() -> Self {
		/* Safety: pool is null */
		unsafe { Self::new_with_pool(Ptr::null()) }
	}

	/// Safety: pool must be either valid for this executor (implementation
	/// defined) or null
	pub unsafe fn new_with_pool(pool: Ptr<Pool>) -> Self {
		Self {
			pool,
			main: Worker::main(),

			/* current cannot be null, is assigned once pinned */
			current: Cell::new(Ptr::null())
		}
	}

	pub fn new_worker(&self, start: Start) -> Worker {
		if self.pool.is_null() {
			/* Safety: executor's lifetime contract is upheld by the implementation */
			unsafe { Worker::new(self.into(), start) }
		} else {
			/* Safety: pool must be valid */
			let fiber = unsafe { self.pool.as_ref() }.new_fiber(start);

			/* Safety: set_start is called above, and executor's lifetime contract is
			 * upheld by the implementation */
			unsafe { Worker::from_fiber(self.into(), fiber) }
		}
	}

	/// Workers move themselves onto their own stack when
	/// they get started. We have to update our current reference
	/// when they get moved and pinned
	pub(super) unsafe fn worker_pinned(&self, worker: Ptr<Worker>) {
		self.current.set(worker);
	}

	/// Suspend the `worker` and resume on the worker that resumed `worker`
	///
	/// Safety: the passed `worker` must be the current worker running
	pub(super) unsafe fn suspend(&self, worker: Ptr<Worker>) {
		let worker = worker.as_ref();
		let from = worker.caller();

		self.current.set(from);

		#[cfg(debug_assertions)]
		{
			if worker.caller().is_null() {
				panic!("Double suspend detected");
			}

			worker.suspend_to(Ptr::null());
		}

		worker.fiber().switch(from.as_ref().fiber());
	}

	/// Switch from whichever `current` worker is running to the new `worker`
	///
	/// Safety: the passed `worker` must not exist on the worker call stack
	/// Workers cannot resume each other recursively
	pub(super) unsafe fn resume(&self, worker: Ptr<Worker>) {
		let previous = self.current.replace(worker);
		let worker = worker.as_ref();

		#[cfg(debug_assertions)]
		{
			if !worker.caller().is_null() {
				panic!("Loop detected");
			}
		}

		worker.suspend_to(previous);
		previous.as_ref().fiber().switch(worker.fiber());
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
		let from = worker.caller();

		self.current.set(from);

		if pool.is_null() {
			worker.into_inner().exit(from.as_ref().fiber())
		} else {
			worker
				.into_inner()
				.exit_to_pool(from.as_ref().fiber(), pool);
		}
	}
}

unsafe impl Pin for Executor {
	unsafe fn pin(&mut self) {
		self.current.set(Ptr::from(&self.main));
	}
}
