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

		worker.inner().resume(self.current.inner());
	}

	/// Switch from whichever `current` worker is running to the new `worker`
	#[inline(always)]
	pub unsafe fn switch_to(&mut self, mut worker: Handle<Worker>) {
		let mut previous = self.current;

		self.current = worker;

		previous.inner().resume(worker.inner());
	}

	pub(crate) unsafe fn start(
		&mut self, mut worker: Handle<Worker>, start: extern "C" fn(*const ()), arg: *const ()
	) {
		if self.current.is_null() {
			self.current = (&mut self.main).into();
		}

		self.current.inner().start(worker.inner(), start, arg);
	}
}

impl Global for Executor {}
