use std::panic::{catch_unwind, AssertUnwindSafe};

pub use crate::macros::{asynchronous, join, select};
use crate::{
	debug,
	error::*,
	fiber::*,
	future::{self, closure::*, future, Cancel, Complete, Future, Progress, ReqPtr, Request},
	impls::{AsyncFn, AsyncFnMut, AsyncFnOnce},
	macros::{panic_nounwind, unreachable_unchecked},
	opt::hint::*,
	pointer::*,
	runtime, warn
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
/// Must not use the context to suspend in a sync function
///
/// See [`get_context`] for more information
#[asynchronous(task)]
pub unsafe trait Task {
	type Output<'a>;

	async fn run(self) -> Self::Output<'_>;
}

#[asynchronous]
pub trait TaskExtensions: Task + Sized {
	async fn map<F, Output>(self, map: F) -> F::Output
	where
		F: AsyncFnOnce<Output>,
		Self: for<'a> Task<Output<'a> = Output>
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
	T: for<'a> Task<Output<'a> = Output>
{
	context.run(task)
}

/// Block on a `Future`, suspending until it completes
#[asynchronous]
pub async fn block_on<F>(future: F) -> F::Output
where
	F: Future
{
	/* Safety: we are in an async function */
	unsafe { get_context().await.block_on(future) }
}

#[asynchronous]
pub async fn current_budget() -> u32 {
	/* Safety: we are in an async function */
	unsafe { get_context().await }.current_budget() as u32
}

#[asynchronous]
#[allow(clippy::impl_trait_in_params)]
pub async fn acquire_budget(amount: impl Into<Option<u32>>) -> bool {
	let amount: u16 = match amount.into().unwrap_or(1).try_into() {
		Ok(ok) => ok,
		Err(_) => return false
	};

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

	if unlikely(interrupted) {
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
