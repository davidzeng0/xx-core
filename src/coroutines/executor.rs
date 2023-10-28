use super::*;
use crate::fiber::*;

/// Per thread executor, responsible for running worker threads
pub struct Executor {
	pool: Handle<Pool>,
	main: Worker,
	current: Handle<Worker>
}

impl Executor {
	pub fn new() -> Self {
		Self {
			pool: unsafe { Handle::new_null() },
			main: Worker::main(),
			current: unsafe { Handle::new_null() }
		}
	}

	pub fn set_pool(&mut self, pool: Handle<Pool>) {
		self.pool = pool;
	}

	pub fn new_worker(&mut self, start: Start) -> Worker {
		if self.pool.is_null() {
			Worker::new(self.into(), start)
		} else {
			Worker::from_fiber(self.into(), self.pool.new_fiber(start))
		}
	}

	/// Workers move themselves onto their own stack when
	/// they get started. We have to update our current reference
	/// when they get moved and pinned
	pub(super) fn worker_pinned(&mut self, worker: Handle<Worker>) {
		self.current = worker;
	}

	/// Suspend the `worker` and resume on the worker that resumed `worker`
	///
	/// Safety: the passed `worker` must be the current worker running
	#[inline(always)]
	pub(super) unsafe fn suspend(&mut self, mut worker: Handle<Worker>) {
		self.current = worker.from();

		worker.inner().switch(self.current.inner());
	}

	/// Switch from whichever `current` worker is running to the new `worker`
	///
	/// Safety: the passed `worker` must not exist on the worker call stack
	/// Workers cannot resume each other recursively
	#[inline(always)]
	pub(super) unsafe fn resume(&mut self, mut worker: Handle<Worker>) {
		let mut previous = self.current;

		self.current = worker;

		worker.set_resume_to(previous);
		previous.inner().switch(worker.inner());
	}

	/// Start a new worker
	///
	/// Safety: same as resume
	pub(super) unsafe fn start(&mut self, worker: Handle<Worker>) {
		self.resume(worker);
	}

	/// Exit the worker and drop its stack
	///
	/// Safety: same as resume
	pub(super) unsafe fn exit(&mut self, worker: Worker) {
		self.current = worker.from();

		if self.pool.is_null() {
			worker.into_inner().exit(self.current.inner())
		} else {
			worker
				.into_inner()
				.exit_to_pool(self.current.inner(), self.pool);
		}
	}
}

impl Global for Executor {
	unsafe fn pinned(&mut self) {
		self.current = (&mut self.main).into();
	}
}
