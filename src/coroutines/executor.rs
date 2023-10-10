use super::worker::Worker;
use crate::task::env::{Global, Handle};

/// Per thread executor, responsible for running worker threads
pub struct Executor {
	main: Worker,
	current: Handle<Worker>
}

impl Executor {
	pub fn new() -> Self {
		Self {
			main: Worker::main(),
			current: unsafe { Handle::new_null() }
		}
	}

	/// Workers move themselves onto their own stack when
	/// they get started. We have to update our current reference
	/// when they get moved and pinned
	pub(crate) fn worker_pinned(&mut self, worker: Handle<Worker>) {
		self.current = worker;
	}

	/// Suspend the `worker` and resume on the worker that resumed `worker`
	///
	/// Safety: the passed `worker` must be the current worker running
	#[inline(always)]
	pub unsafe fn suspend(&mut self, mut worker: Handle<Worker>) {
		self.current = worker.from();

		worker.inner().switch(self.current.inner());
	}

	/// Switch from whichever `current` worker is running to the new `worker`
	///
	/// Safety: the passed `worker` must not exist on the worker call stack
	/// Workers cannot resume each other recursively
	#[inline(always)]
	pub unsafe fn resume(&mut self, mut worker: Handle<Worker>) {
		let mut previous = self.current;

		self.current = worker;

		worker.set_resume_to(previous);
		previous.inner().switch(worker.inner());
	}

	/// Start a new worker
	///
	/// Safety: same as resume
	pub unsafe fn start(&mut self, worker: Handle<Worker>) {
		self.resume(worker);
	}

	/// Exit the worker and drop its stack
	///
	/// Safety: same as resume
	pub unsafe fn exit(&mut self, worker: Worker) {
		self.current = worker.from();

		worker.into_inner().exit(self.current.inner());
	}
}

impl Global for Executor {
	unsafe fn pinned(&mut self) {
		self.current = (&mut self.main).into();
	}
}
