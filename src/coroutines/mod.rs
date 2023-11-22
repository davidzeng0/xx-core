pub mod closure;
pub mod context;
pub use context::*;
pub mod executor;
pub use executor::*;
pub mod worker;
pub use worker::*;
pub mod spawn;
pub use spawn::*;
pub mod select;
pub use select::*;
pub mod join;
pub use join::*;

pub use crate::{async_fn, async_trait, async_trait_impl};
use crate::{
	error::*,
	fiber::*,
	opt::hint::*,
	pointer::*,
	task::{
		block_on as sync_block_on, closure::CancelClosure, env::*, sync_task, Cancel, Progress,
		Request, RequestPtr, Task as SyncTask
	},
	xx_core
};

/// An async task
pub trait Task {
	type Output;

	fn run(self, context: Handle<Context>) -> Self::Output;
}

#[async_fn]
#[inline(always)]
pub async fn get_context() -> Handle<Context> {
	__xx_internal_async_context
}

#[async_fn]
#[inline(always)]
pub async fn block_on<T: SyncTask>(task: T) -> T::Output {
	get_context().await.block_on(task)
}

#[async_fn]
#[inline(always)]
pub async fn is_interrupted() -> bool {
	get_context().await.interrupted()
}

#[async_fn]
#[inline(always)]
pub async fn check_interrupt() -> Result<()> {
	if unlikely(get_context().await.interrupted()) {
		Err(Error::interrupted())
	} else {
		Ok(())
	}
}

#[async_fn]
pub async fn take_interrupt() -> bool {
	let mut context = get_context().await;
	let interrupted = context.interrupted();

	if unlikely(interrupted) {
		context.clear_interrupt();
	}

	interrupted
}

#[async_fn]
pub async fn check_interrupt_take() -> Result<()> {
	if unlikely(take_interrupt().await) {
		Err(Error::interrupted())
	} else {
		Ok(())
	}
}

/// Creates an interrupt guard
///
/// While this guard is held, any attempt to interrupt
/// the current context will be ignored
#[async_fn]
pub async fn interrupt_guard() -> InterruptGuard {
	InterruptGuard::new(get_context().await)
}
