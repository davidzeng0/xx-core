#![allow(clippy::multiple_unsafe_ops_per_block)]

use std::{
	any::TypeId,
	hash::{DefaultHasher, Hash, Hasher}
};

use super::*;
use crate::{closure::Closure, macros::abort};

/// The environment for an async worker
///
/// # Safety
/// implementations must obey the contracts for the functions
pub unsafe trait Environment: 'static {
	/// Gets the context associated with the worker
	///
	/// This function must never panic
	fn context(&self) -> &Context;

	/// Returns the Environment that owns the Context
	///
	/// This function must never panic
	///
	/// # Safety
	/// the context must be the one contained in this env
	unsafe fn from_context(context: &Context) -> &Self;

	/// Creates a new environment for a new worker
	///
	/// # Safety
	/// `worker` must outlive Self
	/// the runtime and the contained context must be alive while it's executing
	/// this function is unsafe so that Context::run may be safe
	unsafe fn clone(&self, worker: Ptr<Worker>) -> Self;

	/// Returns the executor
	///
	/// The executor must be a valid pointer
	/// This function must never panic
	fn executor(&self) -> Ptr<Executor>;

	/// Manually suspend the worker
	///
	/// # Safety
	/// See Worker::suspend
	unsafe fn suspend(&self) {
		/* Safety: guaranteed by caller */
		unsafe { self.context().suspend() };
	}

	/// Manually resume the worker
	///
	/// # Safety
	/// See Worker::resume
	unsafe fn resume(&self) {
		/* Safety: guaranteed by caller */
		unsafe { self.context().resume() };
	}
}

fn type_for<E>() -> u32
where
	E: 'static
{
	let mut hasher = DefaultHasher::new();

	TypeId::of::<E>().hash(&mut hasher);

	/* comparing i128s is generally slower than u32
	 *
	 * u32 is enough to ensure that two different runtimes
	 * are in fact different
	 */
	#[allow(clippy::cast_possible_truncation)]
	(hasher.finish() as u32)
}

fn run_cancel<C>(arg: MutPtr<()>, _: ()) -> Result<()>
where
	C: Cancel
{
	/* Safety: guaranteed by caller. this function cannot be unsafe because it's
	 * stored in the closure, but should be considered as so */
	unsafe {
		let cancel = arg.cast::<Option<C>>().as_mut().take();

		match cancel {
			Some(cancel) => cancel.run(),
			None => unreachable_unchecked!("Fatal error: cancel is missing")
		}
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
	/// # Safety
	/// the context must be alive while it's executing
	/// this function is unsafe so that Context::run may be safe
	#[must_use]
	pub unsafe fn new<E>(worker: Ptr<Worker>) -> Self
	where
		E: Environment
	{
		Self {
			worker,
			environment: type_for::<E>(),
			inner: UnsafeCell::new(ContextInner { guards: 0, interrupted: false, cancel: None })
		}
	}

	/// Runs async task `T`
	#[inline(always)]
	pub fn run<T>(&self, task: T) -> T::Output
	where
		T: Task
	{
		task.run(self.into())
	}

	/// Safety: same as Worker::suspend
	unsafe fn suspend(&self) {
		/* Safety: guaranteed by caller */
		unsafe { self.worker.as_ref().suspend() };
	}

	/// Safety: same as Worker::resume
	unsafe fn resume(&self) {
		/* Safety: guaranteed by caller */
		unsafe { self.worker.as_ref().resume() };
	}

	/// Runs and blocks on future `F`
	pub fn block_on<F>(&self, future: F) -> F::Output
	where
		F: Future
	{
		let block = |cancel| {
			/* avoid allocations by storing on the stack */
			let mut cancel = Some(cancel);
			let canceller =
				Closure::new(MutPtr::from(&mut cancel).as_unit(), run_cancel::<F::Cancel>);

			/* Safety: context is valid while executing. exclusive unsafe
			 * cell access */
			unsafe {
				self.inner.as_mut().cancel = Some(canceller);
				self.suspend();
			}
		};

		let resume = || {
			/* Safety: context is valid while executing. exclusive unsafe
			 * cell access */
			unsafe {
				self.inner.as_mut().cancel = None;
				self.resume();
			}
		};

		/* Safety: we are blocked until the future completes */
		unsafe { future::block_on(block, resume, future) }
	}

	/// Signals an interrupt to the current task
	///
	/// # Safety
	/// See `Cancel::run`
	pub unsafe fn interrupt(&self) -> Result<()> {
		/* Safety: exclusive unsafe cell access */
		let inner = unsafe { self.inner.as_mut() };
		let already_interrupted = inner.interrupted;

		if !already_interrupted {
			inner.interrupted = true;
		}

		#[allow(clippy::never_loop)]
		loop {
			if inner.guards > 0 {
				break;
			}

			let Some(cancel) = inner.cancel.take() else {
				break;
			};

			/* note: this function may recursively call itself if a task awaits
			 * itself
			 * note: the context may no longer be valid after this call
			 */
			return cancel.call(());
		}

		if !already_interrupted {
			Ok(())
		} else {
			Err(Core::Pending.as_err_with_msg("Interrupt pending"))
		}
	}

	/// Returns true if the worker is being interrupted
	///
	/// In the presence of interrupt guards, this returns false
	pub fn interrupted(&self) -> bool {
		/* Safety: exclusive unsafe cell access */
		let inner = unsafe { self.inner.as_ref() };

		inner.guards == 0 && inner.interrupted
	}

	/// Clears any interrupts or pending interrupts (due to guards) on the
	/// current worker
	pub fn clear_interrupt(&self) {
		/* Safety: exclusive unsafe cell access */
		let inner = unsafe { self.inner.as_mut() };

		if inner.interrupted {
			inner.interrupted = false;
		}
	}

	pub fn get_environment<E>(&self) -> Option<&E>
	where
		E: Environment
	{
		if self.environment == type_for::<E>() {
			/* Safety: type is checked */
			Some(unsafe { E::from_context(self) })
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
		/* Safety: context must be valid. get exclusive mutable access to the inner */
		let inner = unsafe { self.context.as_ref().inner.as_mut() };

		inner.guards = match inner.guards.checked_add_signed(rel) {
			Some(guards) => guards,
			/* this can never happen unless memory corruption. useful to check anyway as it
			 * doesn't have to be fast. since this is unsafe and relies on raw pointers, we abort
			 * instead of panic */
			None => abort!("Interrupt guards count overflowed")
		};
	}

	/// # Safety
	/// context must be valid and outlive Self
	pub(super) unsafe fn new(context: Ptr<Context>) -> Self {
		let this = Self { context };

		/* Safety: contract upheld by caller */
		unsafe { this.update_guard_count(1) };

		this
	}
}

impl Drop for InterruptGuard {
	fn drop(&mut self) {
		/* Safety: guaranteed by creator of interrupt guard */
		unsafe { self.update_guard_count(-1) };
	}
}
