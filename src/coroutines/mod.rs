use std::panic::{catch_unwind, AssertUnwindSafe};

pub use crate::macros::asynchronous;
use crate::{
	debug,
	error::*,
	fiber::*,
	future::{self, closure::*, future, Cancel, Complete, Future, Progress, ReqPtr, Request},
	macros::{abort, unreachable_unchecked, unwrap_panic},
	opt::hint::*,
	pointer::*,
	warn
};

mod branch;
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
	unsafe { context.as_ref() }.run(task)
}

#[asynchronous]
pub async fn block_on<F>(future: F) -> F::Output
where
	F: Future
{
	/* Safety: we are in an async function */
	let context = unsafe { get_context().await.as_ref() };

	context.block_on(future)
}

#[asynchronous]
pub async fn is_interrupted() -> bool {
	/* Safety: we are in an async function */
	unsafe { get_context().await.as_ref() }.interrupted()
}

#[asynchronous]
pub async fn check_interrupt() -> Result<()> {
	if !is_interrupted().await {
		Ok(())
	} else {
		Err(Core::Interrupted.as_err())
	}
}

#[asynchronous]
pub async fn clear_interrupt() {
	/* Safety: we are in an async function */
	unsafe { get_context().await.as_ref() }.clear_interrupt();
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
		Err(Core::Interrupted.as_err())
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
