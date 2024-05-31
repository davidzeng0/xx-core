pub use crate::macros::{asynchronous, join, select};
use crate::{
	debug,
	error::*,
	fiber::*,
	future::{self, closure::*, future, Cancel, Complete, Future, Progress, ReqPtr, Request},
	impls::async_fn::*,
	macros::{assert_unsafe_precondition, panic_nounwind, unreachable_unchecked},
	opt::hint::*,
	pointer::*,
	runtime::{self, call_no_unwind, catch_unwind_safe, MaybePanic},
	warn
};

mod lang {}

pub mod branch;
use branch::*;

pub mod closure;
pub mod context;
pub mod executor;
pub mod join;
pub mod select;
pub mod spawn;
pub mod worker;

pub use context::*;
pub use executor::*;
pub use join::*;
pub use select::*;
pub use spawn::*;
pub use worker::*;

/* The budget for async tasks */
pub const DEFAULT_BUDGET: u32 = 128;

/// An async task
///
/// # Safety
/// Exposes the context for the current async worker
/// Must not use the context to suspend in a sync function,
/// unless it's permitted
///
/// See [`get_context`] for more information
#[asynchronous(task)]
#[must_use = "Task does nothing until you `.await` it"]
pub unsafe trait Task {
	type Output<'ctx>;

	async fn run(self) -> Self::Output<'_>;
}

pub mod internal {
	use super::*;

	pub fn as_task<T, Output>(task: T) -> impl for<'ctx> Task<Output<'ctx> = Output>
	where
		T: for<'ctx> Task<Output<'ctx> = Output>
	{
		task
	}
}

#[asynchronous]
pub trait TaskExtensions: Task + Sized {
	async fn map<F, Output>(self, map: F) -> F::Output
	where
		F: AsyncFnOnce<Output>,
		Self: for<'ctx> Task<Output<'ctx> = Output>
	{
		map.call_once(self.await).await
	}
}

impl<T: Task> TaskExtensions for T {}

/// # Safety
/// This function is marked as unsafe because getting
/// a handle to the `Context` would let users suspend in
/// sync functions or closures, which is unsafe
///
/// `'current` is a lifetime that is valid for the current async function, and
/// as such cannot be returned from the function
///
/// To return a lifetime referencing the context, add
/// `#[context('ctx)]` to the function's attributes,
/// and use `'ctx` to reference the lifetime
///
/// See also [`scoped`]
#[asynchronous]
#[lang = "get_context"]
pub async unsafe fn get_context() -> &'current Context {
	/* compiler builtin */
}

/// # Safety
/// The current routine must be allowed to suspend
/// This must be allowed in any async function
///
/// In synchronous functions, it is allowed to suspend
/// if all references are allowed to cross an await barrier
pub unsafe fn scoped<T, Output>(context: &Context, task: T) -> Output
where
	T: for<'ctx> Task<Output<'ctx> = Output>
{
	context.run(task)
}

/// Block on a `Future`, suspending until it completes
#[asynchronous]
#[inline]
pub async fn block_on<F>(future: F) -> F::Output
where
	F: Future
{
	/* Safety: we are in an async function */
	unsafe { get_context().await }.block_on(future, false)
}

/// Runs and blocks on future `F` which is expected to be completed from
/// another thread
///
/// # Panics
/// If the operation isn't supported by the current async runtime
#[asynchronous]
#[inline]
pub async fn block_on_thread_safe<F>(future: F) -> F::Output
where
	F: Future
{
	/* Safety: we are in an async function */
	unsafe { get_context().await }.block_on(future, true)
}

#[asynchronous]
pub async fn current_budget() -> u32 {
	/* Safety: we are in an async function */
	unsafe { get_context().await }.current_budget() as u32
}

#[asynchronous]
#[allow(clippy::impl_trait_in_params)]
pub async fn acquire_budget(amount: impl Into<Option<u32>>) -> bool {
	let amount = amount.into().unwrap_or(1).try_into().unwrap_or(u16::MAX);

	/* Safety: we are in an async function */
	unsafe { get_context().await }
		.decrease_budget(amount)
		.is_some()
}

#[asynchronous]
pub async fn is_interrupted() -> bool {
	/* Safety: we are in an async function */
	unsafe { get_context().await }.interrupted()
}

#[asynchronous]
pub async fn check_interrupt() -> Result<()> {
	if !is_interrupted().await {
		Ok(())
	} else {
		Err(Core::interrupted().into())
	}
}

#[asynchronous]
pub async fn clear_interrupt() {
	/* Safety: we are in an async function */
	unsafe { get_context().await }.clear_interrupt();
}

#[asynchronous]
pub async fn take_interrupt() -> bool {
	let interrupted = is_interrupted().await;

	if interrupted {
		clear_interrupt().await;
	}

	interrupted
}

#[asynchronous]
pub async fn check_interrupt_take() -> Result<()> {
	if !take_interrupt().await {
		Ok(())
	} else {
		Err(Core::interrupted().into())
	}
}

/// Creates an interrupt guard
///
/// While this guard is held, any attempt to interrupt
/// the current context will be ignored
#[asynchronous]
#[context('current)]
pub async fn interrupt_guard() -> InterruptGuard<'current> {
	/* Safety: we are in an async function */
	InterruptGuard::new(unsafe { get_context().await })
}
