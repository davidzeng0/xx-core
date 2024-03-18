#![allow(clippy::multiple_unsafe_ops_per_block)]

use std::cell::Cell;

use super::*;

/// Per thread executor, responsible for running worker threads
pub struct Executor {
	pool: Ptr<Pool>,
	main: Worker,
	current: Cell<Ptr<Worker>>
}

impl Executor {
	#[allow(clippy::new_without_default)]
	#[must_use]
	pub fn new() -> Self {
		/* Safety: pool is null */
		unsafe { Self::new_with_pool(Ptr::null()) }
	}

	/// # Safety
	/// `pool` must be either valid for this executor or null
	#[must_use]
	pub unsafe fn new_with_pool(pool: Ptr<Pool>) -> Self {
		Self {
			pool,
			main: Worker::main(),

			/* current is assigned once pinned */
			current: Cell::new(Ptr::null())
		}
	}

	/// # Safety
	/// Executor must outlive the worker
	pub unsafe fn new_worker(&self, start: Start) -> Worker {
		if self.pool.is_null() {
			/* Safety: guaranteed by caller */
			unsafe { Worker::new(self.into(), start) }
		} else {
			/* Safety: pool must be valid for this executor */
			let fiber = unsafe { self.pool.as_ref() }.new_fiber(start);

			/* Safety: guaranteed by caller */
			unsafe { Worker::from_fiber(self.into(), fiber) }
		}
	}

	/// Workers move themselves onto their own stack when
	/// they get started. We have to update our current reference
	/// when they get moved and pinned
	///
	/// # Safety
	/// the pointer must be the worker that was just started from this executor
	/// and pinned the worker must be alive as long as it's executing
	pub(super) unsafe fn worker_pinned(&self, worker: Ptr<Worker>) {
		self.current.set(worker);
	}

	/// Suspend the current `worker` and resume on the calling worker
	///
	/// # Safety
	/// the passed `worker` must be currently running and started from this
	/// executor
	pub(super) unsafe fn suspend(&self, worker: Ptr<Worker>) {
		/* Safety: worker is valid */
		let worker = unsafe { worker.as_ref() };
		let from = worker.caller();

		self.current.set(from);

		#[cfg(debug_assertions)]
		{
			if worker.caller().is_null() {
				/* Safety: a worker is never double suspended */
				abort!("Double suspend detected");
			}

			/* Safety: clear the caller */
			unsafe { worker.suspend_to(Ptr::null()) };
		}

		/* Safety: workers are alive as long as they're executing */
		unsafe { worker.fiber().switch(from.as_ref().fiber()) };
	}

	/// Switch from whichever `current` worker is running to the new `worker`
	///
	/// # Safety:
	/// the passed `worker` must not exist on the worker call stack
	/// as workers cannot resume each other recursively.
	///
	/// the worker must be started from this executor
	pub(super) unsafe fn resume(&self, worker: Ptr<Worker>) {
		let previous = self.current.replace(worker);

		/* Safety: worker is valid */
		let worker = unsafe { worker.as_ref() };

		#[cfg(debug_assertions)]
		{
			if !worker.caller().is_null() {
				abort!("Loop detected");
			}
		}

		/* Safety: previous resumed worker */
		unsafe { worker.suspend_to(previous) };

		/* Safety: workers must be alive as long as they're executing */
		unsafe { previous.as_ref().fiber().switch(worker.fiber()) };
	}

	/// Start a new worker
	///
	/// # Safety
	/// same as resume
	/// the worker must be alive until it exits
	pub(super) unsafe fn start(&self, worker: Ptr<Worker>) {
		/* Safety: guaranteed by caller */
		unsafe { self.resume(worker) };
	}

	/// Exit the worker and drop its stack
	///
	/// # Safety
	/// same as suspend
	/// the worker must be finished executing
	pub(super) unsafe fn exit(&self, worker: Worker) {
		let pool = self.pool;
		let from = worker.caller();

		self.current.set(from);

		/* Safety: guaranteed by caller */
		unsafe {
			if pool.is_null() {
				worker.into_inner().exit(from.as_ref().fiber());
			} else {
				worker
					.into_inner()
					.exit_to_pool(from.as_ref().fiber(), pool);
			}
		}
	}
}

impl Pin for Executor {
	unsafe fn pin(&mut self) {
		self.current.set(Ptr::from(&self.main));
	}
}
