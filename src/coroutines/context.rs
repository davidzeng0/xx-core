#![allow(clippy::multiple_unsafe_ops_per_block)]

use std::{
	any::TypeId,
	hash::{DefaultHasher, Hash, Hasher}
};

use super::*;

/// The environment for an async worker
///
/// # Safety
/// implementations must obey the contracts for implementing the functions
pub unsafe trait Environment: 'static {
	/// Gets the context associated with the worker
	///
	/// This function must never panic, and must return the same context every
	/// time
	fn context(&self) -> &Context;

	/// Gets the context associated with the worker
	///
	/// This function must never panic, and must return the same context as
	/// `Environment::context`
	fn context_mut(&mut self) -> &mut Context;

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
	/// the environment and the contained context must be alive while it's
	/// executing this function is unsafe so that Context::run may be safe
	unsafe fn clone(&self) -> Self;

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

	/* comparing `TypeId`s is generally slower than u32
	 *
	 * u32 is enough to ensure that two different environments
	 * are in fact different
	 */
	#[allow(clippy::cast_possible_truncation)]
	(hasher.finish() as u32)
}

/// # Safety
/// the future must be running
/// `arg` must be a pointer to `Option<C>`
unsafe fn run_cancel<C>(arg: MutPtr<()>) -> Result<()>
where
	C: Cancel
{
	/* Safety: guaranteed by caller */
	unsafe {
		let cancel = ptr!(arg.cast::<Option<C>>()=>take());

		match cancel {
			Some(cancel) => cancel.run(),
			None => unreachable_unchecked!("Fatal error: cancel is missing")
		}
	}
}

struct Canceller(MutPtr<()>, unsafe fn(MutPtr<()>) -> Result<()>);

impl Canceller {
	const fn new<C: Cancel>(cancel: MutPtr<Option<C>>) -> Self {
		Self(cancel.cast(), run_cancel::<C>)
	}

	/// # Safety
	/// See `Cancel::run`
	unsafe fn call(self) -> Result<()> {
		let Self(arg, cancel) = self;

		/* Safety: guaranteed by caller */
		unsafe { cancel(arg) }
	}
}

struct ContextInner {
	budget: u16,
	guards: u32,
	interrupted: bool,
	cancel: Option<Canceller>
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
	///
	/// must call `set_worker`
	#[must_use]
	pub unsafe fn new<E>() -> Self
	where
		E: Environment
	{
		Self {
			worker: Ptr::null(),
			environment: type_for::<E>(),
			inner: UnsafeCell::new(ContextInner {
				#[allow(clippy::cast_possible_truncation)]
				budget: DEFAULT_BUDGET as u16,
				guards: 0,
				interrupted: false,
				cancel: None
			})
		}
	}

	/// # Safety
	/// worker must be a valid pointer, and must outlive this context
	pub unsafe fn set_worker(&mut self, worker: Ptr<Worker>) {
		self.worker = worker;
	}

	/// Runs async task `T`
	#[inline(always)]
	pub fn run<T>(&self, task: T) -> T::Output<'_>
	where
		T: Task
	{
		task.run(self)
	}

	/// Safety: same as Worker::suspend
	unsafe fn suspend(&self) {
		/* Safety: guaranteed by caller */
		unsafe { ptr!(self.worker=>suspend()) };
	}

	/// Safety: same as Worker::resume
	unsafe fn resume(&self) {
		/* Safety: guaranteed by caller */
		unsafe { ptr!(self.worker=>resume()) };
	}

	/// Runs and blocks on future `F`
	pub fn block_on<F>(&self, future: F) -> F::Output
	where
		F: Future
	{
		let worker = self.worker;

		let block = move |cancel| {
			/* avoid allocations by storing on the stack */
			let mut cancel = Some(cancel);
			let canceller = Canceller::new(ptr!(&mut cancel));

			/* Safety: context is valid while executing
			 * exclusive unsafe cell access
			 */
			unsafe {
				let inner = self.inner.as_mut();

				#[allow(clippy::cast_possible_truncation)]
				(inner.budget = DEFAULT_BUDGET as u16);
				inner.cancel = Some(canceller);

				self.suspend();

				ptr!(self.inner=>cancel = None);
			}
		};

		let resume = move || {
			/* Safety: context is valid while executing */
			unsafe { ptr!(worker=>resume()) };
		};

		/* Safety: we are blocked until the future completes */
		unsafe { future::block_on(block, resume, future) }
	}

	pub fn current_budget(&self) -> u16 {
		/* Safety: exclusive unsafe cell access */
		unsafe { ptr!(self.inner=>budget) }
	}

	pub fn decrease_budget(&self, amount: u16) -> Option<u16> {
		/* Safety: exclusive unsafe cell access */
		let inner = unsafe { self.inner.as_mut() };
		let result = inner.budget.checked_sub(amount);

		inner.budget = result.unwrap_or(0);
		result
	}

	/// Signals an interrupt to the current task
	///
	/// # Safety
	/// See `Cancel::run`
	pub unsafe fn interrupt(this: Ptr<Self>) -> Result<()> {
		/* Safety: exclusive unsafe cell access */
		let inner = unsafe { ptr!(this=>inner.as_mut()) };
		let already_interrupted = inner.interrupted;

		if !already_interrupted {
			inner.interrupted = true;
		}

		#[allow(clippy::never_loop)]
		loop {
			if inner.guards > 0 {
				break;
			}

			let Some(canceller) = inner.cancel.take() else {
				break;
			};

			/* this function may recursively call itself if a task
			 * is awaiting itself
			 *
			 * the context may no longer be valid after this call
			 *
			 * `inner` transitions to Disabled, which is okay
			 * because it's not a protected tag
			 *
			 * Safety: guaranteed by caller
			 */
			return unsafe { canceller.call() };
		}

		if !already_interrupted {
			Ok(())
		} else {
			Err(Core::Pending("Interrupt pending".into()).into())
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
			Some(runtime::call_non_panicking(|| unsafe {
				E::from_context(self)
			}))
		} else {
			None
		}
	}
}

pub struct InterruptGuard<'a> {
	context: &'a Context
}

impl<'a> InterruptGuard<'a> {
	fn update_guard_count(&self, rel: i32) {
		/* Safety: exclusive unsafe cell access */
		let inner = unsafe { self.context.inner.as_mut() };

		inner.guards = match inner.guards.checked_add_signed(rel) {
			Some(guards) => guards,
			/* this can never happen unless memory corruption. useful to check anyway as it
			 * doesn't have to be fast. since this is unsafe and relies on raw pointers, we abort
			 * instead of panic */
			None => panic_nounwind!("Interrupt guards count overflowed")
		};
	}

	pub(super) fn new(context: &'a Context) -> Self {
		let this = Self { context };

		this.update_guard_count(1);
		this
	}
}

impl Drop for InterruptGuard<'_> {
	fn drop(&mut self) {
		self.update_guard_count(-1);
	}
}
