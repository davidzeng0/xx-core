use super::worker::Worker;
use crate::task::env::{Global, Handle};

/// Per thread executor, responsible for running worker threads
pub struct Executor {
	main: Worker,
	current: Handle<Worker>
}

impl Executor {
	pub fn new() -> Executor {
		Executor {
			main: Worker::main(),
			current: unsafe { Handle::new_empty() }
		}
	}

	/// Suspend the `worker` and resume on `main`
	#[inline(always)]
	pub unsafe fn suspend(&mut self, mut worker: Handle<Worker>) {
		self.current = (&mut self.main).into();

		worker.inner().switch(self.current.inner());
	}

	/// Switch from whichever `current` worker is running to the new `worker`
	#[inline(always)]
	pub unsafe fn switch_to(&mut self, mut worker: Handle<Worker>) {
		let mut previous = self.current;

		self.current = worker;

		previous.inner().switch(worker.inner());
	}

	pub(crate) unsafe fn start(&mut self, mut worker: Handle<Worker>) {
		if self.current.is_null() {
			self.current = (&mut self.main).into();
		}

		self.current.inner().switch(worker.inner());
	}

	pub(crate) unsafe fn exit(&mut self, worker: Worker) {
		self.current = (&mut self.main).into();

		worker.into_inner().exit(self.current.inner());
	}
}

impl Global for Executor {}
