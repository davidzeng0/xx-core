use std::panic::{catch_unwind, AssertUnwindSafe};

pub use crate::macros::{asynchronous, join, select};
use crate::{
	debug,
	error::*,
	fiber::*,
	future::{self, closure::*, future, Cancel, Complete, Future, Progress, ReqPtr, Request},
	impls::AsyncFnOnce,
	macros::{panic_nounwind, unreachable_unchecked},
	opt::hint::*,
	pointer::*,
	runtime, warn
};

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

/// An async task
pub trait Task {
	type Output;

	/// This function is marked as safe because it will be converted to a
	/// reference in the future. However, it is not safe right now. Do not call
	/// this function without a valid context
	fn run(self, context: Ptr<Context>) -> Self::Output;
}

#[asynchronous]
pub trait TaskExtensions: Task + Sized {
	async fn map<F>(self, map: F) -> F::Output
	where
		F: AsyncFnOnce<Self::Output>
	{
		map.call_once(self.await).await
	}
}

impl<T: Task> TaskExtensions for T {}

/// Get a pointer to the current context
///
/// Always returns a valid, dereferenceable pointer if the calling function is
/// an async function. Otherwise, the context must be valid until it has
/// finished executing.
#[asynchronous]
#[lang = "get_context"]
pub async fn get_context() -> Ptr<Context> {
	/* compiler builtin */
}

/// # Safety
/// `context` and `task` must live across suspends, and any lifetimes
/// captured by `task` must remain valid until this function returns
pub unsafe fn with_context<T>(context: Ptr<Context>, task: T) -> T::Output
where
	T: Task
{
	/* Safety: guaranteed by caller */
	unsafe { ptr!(context=>run(task)) }
}

#[asynchronous]
pub async fn block_on<F>(future: F) -> F::Output
where
	F: Future
{
	let context = get_context().await;

	/* Safety: we are in an async function */
	unsafe { ptr!(context=>block_on(future)) }
}

#[asynchronous]
pub async fn current_budget() -> u32 {
	let context = get_context().await;

	/* Safety: we are in an async function */
	unsafe { ptr!(context=>current_budget() as u32) }
}

#[asynchronous]
pub async fn acquire_budget(amount: Option<u32>) -> bool {
	let amount: u16 = match amount.unwrap_or(1).try_into() {
		Ok(ok) => ok,
		Err(_) => return false
	};

	let context = get_context().await;

	/* Safety: we are in an async function */
	unsafe { ptr!(context=>decrease_budget(amount).is_some()) }
}

#[asynchronous]
pub async fn is_interrupted() -> bool {
	let context = get_context().await;

	/* Safety: we are in an async function */
	unsafe { ptr!(context=>interrupted()) }
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
	let context = get_context().await;

	/* Safety: we are in an async function */
	unsafe { ptr!(context=>clear_interrupt()) };
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
///
/// Safety: the async context must outlive InterruptGuard. Since InterruptGuard
/// does not have a lifetime generic, care should be taken to ensure that it
/// doesn't get dropped after the async context gets dropped
///
/// This usually never happens unless InterruptGuard is moved into a struct
#[asynchronous]
pub async unsafe fn interrupt_guard() -> InterruptGuard {
	/* Safety: guaranteed by caller */
	unsafe { InterruptGuard::new(get_context().await) }
}
