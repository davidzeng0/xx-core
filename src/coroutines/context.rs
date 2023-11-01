use std::any::TypeId;

use super::*;
use crate::{closure::Closure, task::block_on};

fn run_cancel<C: Cancel>(arg: *const (), _: ()) -> Result<()> {
	let mut cancel: MutPtr<Option<C>> = ConstPtr::from(arg).cast();

	unsafe { cancel.take().unwrap_unchecked().run() }
}

pub trait PerContextRuntime: Global + 'static {
	fn context(&mut self) -> &mut Context;

	fn from_context(context: &mut Context) -> &mut Self;

	fn new_from_worker(&mut self, worker: Handle<Worker>) -> Self;

	fn executor(&mut self) -> Handle<Executor> {
		self.context().executor()
	}

	unsafe fn suspend(&mut self) {
		self.context().suspend()
	}

	unsafe fn resume(&mut self) {
		self.context().resume()
	}
}

pub struct Context {
	executor: Handle<Executor>,
	worker: Handle<Worker>,

	cancel: Option<Closure<*const (), (), Result<()>>>,

	guards: u32,
	interrupted: bool,

	runtime_type: u32
}

fn type_for<R: PerContextRuntime>() -> u32 {
	let id: i128 = unsafe { std::mem::transmute(TypeId::of::<R>()) };

	/* comparing i128s is generally slower than u32
	 *
	 * u32 is enough to ensure that two different runtimes
	 * are in fact different
	 */
	id as u32
}

impl Context {
	pub fn new<R: PerContextRuntime>(executor: Handle<Executor>, worker: Handle<Worker>) -> Self {
		Self {
			executor,
			worker,

			cancel: None,

			guards: 0,
			interrupted: false,

			runtime_type: type_for::<R>()
		}
	}

	fn executor(&mut self) -> Handle<Executor> {
		self.executor
	}

	#[inline(always)]
	fn suspend(&mut self) {
		unsafe {
			self.worker.suspend();
		}
	}

	#[inline(always)]
	fn resume(&mut self) {
		unsafe {
			self.executor.resume(self.worker);
		}
	}

	/// Runs async task `T`
	#[inline(always)]
	pub fn run<T: Task>(&mut self, task: T) -> T::Output {
		task.run(self.into())
	}

	/// Runs and blocks on sync task `T`
	#[inline(always)]
	pub fn block_on<T: SyncTask>(&mut self, task: T) -> T::Output {
		let handle = Handle::from(self);

		block_on(
			|cancel| {
				/* hold variably sized cancel on the stack,
				 * in an option so that we know it's been
				 * moved when `interrupt` is called
				 *
				 * we have to use a specialized function for each
				 * cancel type
				 *
				 * this removes the need to allocate memory
				 * to box this cancel, potentially causing
				 * significant slowdowns
				 */
				let mut cancel = Some(cancel);

				handle.clone().cancel = Some(Closure::new(
					MutPtr::from(&mut cancel).as_raw_ptr(),
					run_cancel::<T::Cancel>
				));

				handle.clone().suspend();
			},
			|| {
				handle.clone().resume();
			},
			task
		)
	}

	/// Interrupt the current running task
	pub fn interrupt(&mut self) -> Result<()> {
		self.interrupted = true;

		if self.guards == 0 {
			self.interrupted = true;
			self.cancel
				.take()
				.expect("Task interrupted while active")
				.call(())
		} else {
			Err(Error::new(ErrorKind::Other, "Interrupt queued"))
		}
	}

	/// Returns true if the worker is being interrupted
	///
	/// In the presence of interrupt guards, this returns false
	pub fn interrupted(&self) -> bool {
		if likely(self.guards == 0) {
			self.interrupted
		} else {
			false
		}
	}

	/// Clears any interrupts or pending interrupts (due to guards) on the
	/// current worker
	pub fn clear_interrupt(&mut self) {
		self.interrupted = false;
	}

	#[inline(always)]
	pub fn get_runtime<R: PerContextRuntime>(&mut self) -> Option<Handle<R>> {
		if self.runtime_type == type_for::<R>() {
			Some(R::from_context(self).into())
		} else {
			None
		}
	}
}

impl Global for Context {}

pub struct InterruptGuard {
	context: Handle<Context>
}

impl InterruptGuard {
	fn update_guard_count(&mut self, rel: i32) {
		self.context.guards = self
			.context
			.guards
			.checked_add_signed(rel)
			.expect("Interrupt guards count overflowed");
	}

	pub(super) fn new(context: Handle<Context>) -> Self {
		let mut this = Self { context };

		this.update_guard_count(1);
		this
	}
}

impl Drop for InterruptGuard {
	fn drop(&mut self) {
		self.update_guard_count(-1);
	}
}
