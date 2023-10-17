use super::{async_fn, env::AsyncContext};
use crate::{
	error::{Error, Result},
	task::{env::Handle, Cancel, Task},
	xx_core
};

#[async_fn]
pub async fn get_context<Context: AsyncContext>() -> Handle<Context> {
	__xx_internal_async_context
}

#[async_fn]
pub async fn block_on<Context: AsyncContext, T: Task<Output, C>, C: Cancel, Output>(
	task: T
) -> Output {
	get_context().await.block_on(task)
}

#[async_fn]
pub async fn is_interrupted<Context: AsyncContext>() -> bool {
	get_context().await.interrupted()
}

#[async_fn]
pub async fn check_interrupt<Context: AsyncContext>() -> Result<()> {
	if get_context().await.interrupted() {
		Err(Error::interrupted())
	} else {
		Ok(())
	}
}

#[async_fn]
pub async fn take_interrupt<Context: AsyncContext>() -> bool {
	let mut context = get_context().await;
	let interrupted = context.interrupted();

	if interrupted {
		context.clear_interrupt();
	}

	interrupted
}

#[async_fn]
pub async fn check_interrupt_take<Context: AsyncContext>() -> Result<()> {
	if take_interrupt().await {
		Err(Error::interrupted())
	} else {
		Ok(())
	}
}

pub struct InterruptGuard<Context: AsyncContext> {
	context: Handle<Context>
}

impl<Context: AsyncContext> InterruptGuard<Context> {
	fn new(mut context: Handle<Context>) -> Self {
		context.interrupt_guard(1);

		Self { context }
	}
}

impl<Context: AsyncContext> Drop for InterruptGuard<Context> {
	fn drop(&mut self) {
		self.context.interrupt_guard(-1);
	}
}

/// Creates an interrupt guard
///
/// While this guard is held, any attempt to interrupt
/// the current context will be ignored
#[async_fn]
pub async fn interrupt_guard<Context: AsyncContext>() -> InterruptGuard<Context> {
	InterruptGuard::new(get_context().await)
}
