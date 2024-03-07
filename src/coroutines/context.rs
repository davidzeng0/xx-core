use std::{any::TypeId, mem::transmute};

use super::*;
use crate::closure::Closure;

/// The environment for a async worker
pub trait Environment: 'static {
	/// Gets the context associated with the worker
	fn context(&self) -> &Context;

	/// Returns the PerContextRuntime that owns the Context
	fn from_context(context: &Context) -> Ptr<Self>;

	/// Creates a new environment for a new worker
	unsafe fn clone(&self, worker: Ptr<Worker>) -> Self;

	fn executor(&self) -> Ptr<Executor>;

	/// Manually suspend the worker
	unsafe fn suspend(&self) {
		self.context().suspend()
	}

	/// Manually resume the worker
	unsafe fn resume(&self) {
		self.context().resume()
	}
}

fn type_for<R: Environment>() -> u32 {
	let id: u128 = unsafe { transmute(TypeId::of::<R>()) };

	/* comparing i128s is generally slower than u32
	 *
	 * u32 is enough to ensure that two different runtimes
	 * are in fact different
	 */
	id as u32
}

fn run_cancel<C: Cancel>(arg: MutPtr<()>, _: ()) -> Result<()> {
	unsafe {
		let cancel = arg.cast::<Option<C>>().as_mut();

		cancel.take().unwrap().run()
	}
}

struct ContextInner {
	guards: u32,
	interrupted: bool,
	cancel: Option<Closure<MutPtr<()>, (), Result<()>>>
}

pub struct Context {
	worker: Ptr<Worker>,
	environment: u32,
	inner: UnsafeCell<ContextInner>
}

impl Context {
	pub fn new<R: Environment>(worker: Ptr<Worker>) -> Self {
		Self {
			worker,
			environment: type_for::<R>(),
			inner: UnsafeCell::new(ContextInner { guards: 0, interrupted: false, cancel: None })
		}
	}

	/// Runs async task `T`
	#[inline(always)]
	pub fn run<T: Task>(&self, task: T) -> T::Output {
		task.run(self.into())
	}

	/// Safety: same as Worker::suspend
	unsafe fn suspend(&self) {
		self.worker.as_ref().suspend();
	}

	/// Safety: same as Worker::resume
	unsafe fn resume(&self) {
		self.worker.as_ref().resume();
	}

	/// Runs and blocks on future `T`
	pub fn block_on<T: SyncTask>(&self, task: T) -> T::Output {
		let block = |cancel| {
			/* avoid allocations by storing on the stack */
			let mut cancel = Some(cancel);
			let canceller =
				Closure::new(MutPtr::from(&mut cancel).as_unit(), run_cancel::<T::Cancel>);

			/* Safety: contract upheld by the implementation */
			unsafe {
				self.inner.as_mut().cancel = Some(canceller);
				self.suspend();
				self.inner.as_mut().cancel = None;
			}
		};

		let resume = || {
			/* Safety: contract upheld by the implementation */
			unsafe { self.resume() };
		};

		/* Safety: we are blocked until the task completes */
		unsafe { sync_block_on(block, resume, task) }
	}

	/// Interrupt the current running task
	pub fn interrupt(&self) -> Result<()> {
		let inner = unsafe { self.inner.as_mut() };

		inner.interrupted = true;

		if inner.guards == 0 {
			inner.interrupted = true;
			inner
				.cancel
				.take()
				.expect("Task interrupted while active")
				.call(())
		} else {
			Err(Core::Pending.as_err_with_msg("Interrupt pending"))
		}
	}

	/// Returns true if the worker is being interrupted
	///
	/// In the presence of interrupt guards, this returns false
	pub fn interrupted(&self) -> bool {
		let inner = unsafe { self.inner.as_ref() };

		inner.guards == 0 && inner.interrupted
	}

	/// Clears any interrupts or pending interrupts (due to guards) on the
	/// current worker
	pub fn clear_interrupt(&self) {
		let inner = unsafe { self.inner.as_mut() };

		inner.interrupted = false;
	}

	pub fn get_runtime<R: Environment>(&self) -> Option<Ptr<R>> {
		if self.environment == type_for::<R>() {
			Some(R::from_context(self.into()))
		} else {
			None
		}
	}
}

pub struct InterruptGuard {
	context: Ptr<Context>
}

impl InterruptGuard {
	/// Safety: self.context must be valid
	unsafe fn update_guard_count(&self, rel: i32) {
		let inner = self.context.as_ref().inner.as_mut();

		inner.guards = inner
			.guards
			.checked_add_signed(rel)
			/* this can never happen unless memory corruption. useful to check anyway as it
			 * doesn't have to be fast */
			.expect("Interrupt guards count overflowed");
	}

	/// Safety: self.context must be valid
	pub(super) unsafe fn new(context: Ptr<Context>) -> Self {
		let this = Self { context };

		/* Safety: contract upheld by caller */
		unsafe { this.update_guard_count(1) };

		this
	}
}

impl Drop for InterruptGuard {
	fn drop(&mut self) {
		unsafe { self.update_guard_count(-1) };
	}
}
