use crate::error::*;
use crate::fiber::*;
use crate::future::closure::*;
use crate::future::{self, future, Cancel, Complete, Future, Progress, ReqPtr, Request};
use crate::macros::{assert_unsafe_precondition, unreachable_unchecked};
pub use crate::macros::{asynchronous, join, select};
use crate::opt::hint::*;
use crate::pointer::*;
use crate::runtime::{self, call_no_unwind, catch_unwind_safe, MaybePanic};
use crate::{debug, trace, warn};

mod lang {}

pub mod branch;
pub mod context;
pub mod executor;
pub mod impls;
pub mod join;
pub mod ops;
pub mod select;
pub mod spawn;
pub mod worker;

pub mod internal;

#[doc(inline)]
pub use {context::*, executor::*, join::*, select::*, spawn::*, worker::*};

use self::branch::*;
use self::ops::*;

/// The budget for async tasks
pub const DEFAULT_BUDGET: u32 = 128;

/// An async task is a value that might not have finished computing yet. This
/// kind of “asynchronous value” makes it possible for a thread to continue
/// doing useful work while it waits for the value to become available
#[asynchronous]
#[lang = "task"]
#[must_use = "Task does nothing until you `.await` it"]
pub trait Task {
	type Output;

	async fn run(self) -> Self::Output;
}

/// Get the current async context
///
/// `'current` is a lifetime that is only valid for the current async function
///
/// To return a lifetime referencing the context, add `#[cx]` to the lifetime
/// you wish to use as the lifetime of the context
///
/// See also [`scoped`]
#[asynchronous]
#[lang = "get_context"]
pub async fn get_context() -> &'current Context {
	/* compiler builtin */
}

/// Runs the async task `T` on the current execution context, as if the caller
/// were an async function `.await`ing a result
///
/// This is useful when interfacing with synchronous code without needing to
/// block the current thread, by suspending the worker when a possibly blocking
/// operation is performed
///
/// # Safety
/// The current execution context (aka call stack and instruction pointer) must
/// belong to the async context passed into this function. It is an error to
/// export the context to another async worker or thread and call this function
///
/// The async task `T` may suspend the current execution context when waiting
/// for an operation to complete. The caller must ensure that either the task
/// never suspends, or if it does, that all references are allowed to cross the
/// await barrier.
///
/// Note that references are still borrowed even when suspended. This means that
/// they must not become dangling, or violate the aliasing rules if another
/// worker acquires a reference to the same data
pub unsafe fn scoped<T, Output>(context: &Context, task: T) -> Output
where
	T: for<'ctx> Task<Output<'ctx> = Output>
{
	/* Safety: guaranteed by caller */
	unsafe { context.run(task) }
}

/// Block on a future `F`, suspending until it completes
#[asynchronous]
#[inline]
pub async fn block_on<F>(future: F) -> F::Output
where
	F: Future
{
	/* Safety: we are in an async function */
	unsafe { get_context().await.block_on(future, false) }
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
	unsafe { get_context().await.block_on(future, true) }
}

#[asynchronous]
pub async fn current_budget() -> u32 {
	get_context().await.current_budget() as u32
}

#[asynchronous]
#[allow(clippy::impl_trait_in_params)]
pub async fn acquire_budget(amount: impl Into<Option<u32>>) -> bool {
	let amount = amount.into().unwrap_or(1).try_into().unwrap_or(u16::MAX);

	get_context().await.decrease_budget(amount).is_some()
}

#[asynchronous]
pub async fn is_interrupted() -> bool {
	get_context().await.interrupted()
}

#[asynchronous]
pub async fn check_interrupt() -> Result<()> {
	if !is_interrupted().await {
		Ok(())
	} else {
		Err(ErrorKind::Interrupted.into())
	}
}

#[asynchronous]
pub async fn clear_interrupt() {
	get_context().await.clear_interrupt();
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
		Err(ErrorKind::Interrupted.into())
	}
}

/// Creates an interrupt guard
///
/// While this guard is held, any attempt to interrupt
/// the current context will be ignored
#[asynchronous]
pub async fn interrupt_guard<#[cx] 'current>() -> InterruptGuard<'current> {
	InterruptGuard::new(get_context().await)
}
