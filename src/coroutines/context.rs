#![allow(clippy::multiple_unsafe_ops_per_block)]

use std::any::TypeId;
use std::hash::{DefaultHasher, Hash, Hasher};

use static_assertions::const_assert;

use super::*;
use crate::impls::{Cell, OptionExt};

/// The environment for an async worker
///
/// # Safety
/// implementations must obey the contracts for implementing the functions
pub unsafe trait Environment: 'static {
	/// Gets the context associated with the worker
	///
	/// This function must never unwind, and must return the same context every
	/// time
	fn context(&self) -> &Context;

	/// Gets the context associated with the worker
	///
	/// This function must never unwind, and must return the same context as
	/// `Environment::context`
	fn context_mut(&mut self) -> &mut Context;

	/// Returns the Environment that owns the Context
	///
	/// This function must never unwind
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
	/// This function must never unwind
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

#[derive(Clone, Copy)]
struct Canceller(MutPtr<()>, unsafe fn(MutPtr<()>) -> Result<()>);

impl Canceller {
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

			cancel
				.expect_unchecked("Fatal error: cancel is missing")
				.run()
		}
	}

	const fn new<C: Cancel>(cancel: MutPtr<Option<C>>) -> Self {
		Self(cancel.cast(), Self::run_cancel::<C>)
	}

	/// # Safety
	/// See `Cancel::run`
	unsafe fn call(self) -> Result<()> {
		let Self(arg, callback) = self;

		/* Safety: guaranteed by caller */
		unsafe { callback(arg) }
	}
}

#[derive(Clone, Copy)]
pub struct WakerVTable {
	prepare: unsafe fn(Ptr<()>),
	wake: unsafe fn(Ptr<()>, ReqPtr<()>)
}

impl WakerVTable {
	/// # Safety
	/// `prepare` must never unwind
	/// `wake` is thread safe and must never unwind
	#[must_use]
	pub const unsafe fn new(
		prepare: unsafe fn(Ptr<()>), wake: unsafe fn(Ptr<()>, ReqPtr<()>)
	) -> Self {
		Self { prepare, wake }
	}
}

#[allow(missing_copy_implementations)]
pub struct Waker {
	ptr: Ptr<()>,
	vtable: &'static WakerVTable
}

impl Waker {
	#[must_use]
	pub const fn new(ptr: Ptr<()>, vtable: &'static WakerVTable) -> Self {
		Self { ptr, vtable }
	}

	/// # Safety
	/// TBD
	pub unsafe fn prepare(&self) {
		/* Safety: guaranteed by caller */
		unsafe { (self.vtable.prepare)(self.ptr) };
	}

	/// # Safety
	/// Must have already called `prepare`
	/// Must only call once when it is ready to wake the task
	pub unsafe fn wake(&self, request: ReqPtr<()>) {
		/* Safety: guaranteed by caller */
		unsafe { (self.vtable.wake)(self.ptr, request) }
	}
}

unsafe fn wake(_: ReqPtr<()>, arg: Ptr<()>, _: ()) {
	let worker = arg.cast::<Worker>();

	/* Safety: guaranteed by caller */
	unsafe { ptr!(worker=>resume()) };
}

struct Data {
	budget: Cell<u16>,
	guards: Cell<u32>,
	interrupted: Cell<bool>,
	cancel: Cell<Option<Canceller>>
}

impl Data {
	const fn new() -> Self {
		const_assert!(DEFAULT_BUDGET <= u16::MAX as u32);

		Self {
			#[allow(clippy::cast_possible_truncation)]
			budget: Cell::new(DEFAULT_BUDGET as u16),
			guards: Cell::new(0),
			interrupted: Cell::new(false),
			cancel: Cell::new(None)
		}
	}
}

#[repr(C)]
pub struct Context {
	environment: u32,
	worker: Ptr<Worker>,
	waker: Option<Waker>,
	data: Data
}

impl Context {
	/// # Safety
	/// the context must be alive while it's executing
	/// this function is unsafe so that Context::run may be safe
	///
	/// must call `set_worker`
	#[must_use]
	pub unsafe fn new<E>(waker: Option<Waker>) -> Self
	where
		E: Environment
	{
		Self {
			environment: type_for::<E>(),
			worker: Ptr::null(),
			waker,
			data: Data::new()
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
	///
	/// # Panics
	/// If a thread safe block was requested and isn't supported
	#[inline]
	pub fn block_on<F>(&self, future: F, thread_safe: bool) -> F::Output
	where
		F: Future
	{
		let block = move |cancel| {
			/* avoid allocations by storing on the stack */
			let mut cancel = Some(cancel);
			let canceller = Canceller::new(ptr!(&mut cancel));

			#[allow(clippy::cast_possible_truncation)]
			self.data.budget.set(DEFAULT_BUDGET as u16);
			self.data.cancel.set(Some(canceller));

			/* Safety: context is valid while executing */
			unsafe { self.suspend() };

			self.data.cancel.set(None);
		};

		if thread_safe {
			#[allow(clippy::expect_used)]
			let waker = self.waker.as_ref().expect("Operation not supported");

			/* Safety: prepare for wake */
			unsafe { waker.prepare() };

			/* Safety: wake doesn't unwind */
			let request = unsafe { Request::new(self.worker.cast(), wake) };
			let req_ptr = ptr!(&request);

			/* Safety: context is valid while executing */
			let resume = move || unsafe { waker.wake(req_ptr) };

			/* Safety: we are blocked until the future completes */
			unsafe { future::block_on(block, resume, future) }
		} else {
			let worker = self.worker;

			/* Safety: context is valid while executing */
			let resume = move || unsafe { ptr!(worker=>resume()) };

			/* Safety: we are blocked until the future completes */
			unsafe { future::block_on(block, resume, future) }
		}
	}

	pub fn current_budget(&self) -> u16 {
		self.data.budget.get()
	}

	pub fn decrease_budget(&self, amount: u16) -> Option<u16> {
		let result = self.data.budget.get().checked_sub(amount);

		self.data.budget.set(result.unwrap_or(0));

		result
	}

	/// Signals an interrupt to the current task
	///
	/// # Safety
	/// See `Cancel::run`
	pub unsafe fn interrupt(this: Ptr<Self>) -> Result<()> {
		/* Safety: guaranteed by caller */
		let this = unsafe { this.as_ref() };
		let interrupted = this.data.interrupted.replace(true);

		#[allow(clippy::never_loop)]
		loop {
			if this.data.guards.get() > 0 {
				break;
			}

			let Some(canceller) = this.data.cancel.take() else {
				break;
			};

			/* this function may recursively call itself if a task
			 * is awaiting itself
			 *
			 * the context may no longer be valid after this call
			 *
			 * Safety: guaranteed by caller
			 */
			return unsafe { canceller.call() };
		}

		if !interrupted {
			Ok(())
		} else {
			Err(ErrorKind::AlreadyInProgress.into())
		}
	}

	/// Returns true if the worker is being interrupted
	///
	/// In the presence of interrupt guards, this returns false
	pub fn interrupted(&self) -> bool {
		self.data.guards.get() == 0 && self.data.interrupted.get()
	}

	/// Clears any interrupts or pending interrupts (due to guards) on the
	/// current worker
	pub fn clear_interrupt(&self) {
		self.data.interrupted.set(false);
	}

	pub fn get_environment<E>(&self) -> Option<&E>
	where
		E: Environment
	{
		if self.environment == type_for::<E>() {
			/* Safety: type is checked */
			Some(call_no_unwind(|| unsafe { E::from_context(self) }))
		} else {
			None
		}
	}
}

pub struct InterruptGuard<'ctx> {
	context: &'ctx Context
}

impl<'ctx> InterruptGuard<'ctx> {
	fn update_guard_count(&self, rel: i32) {
		self.context.data.guards.update(|guards| {
			guards
				.checked_add_signed(rel)
				/* this can never happen unless memory corruption. useful to check anyway as
				 * it doesn't have to be fast
				 */
				.expect_nounwind("Interrupt guards count overflowed")
		});
	}

	pub(super) fn new(context: &'ctx Context) -> Self {
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
