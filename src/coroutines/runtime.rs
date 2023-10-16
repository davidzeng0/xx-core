use std::io::{Error, ErrorKind, Result};

use super::{async_fn, env::AsyncContext};
use crate::{
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
		Err(Error::new(ErrorKind::Interrupted, "Operation canceled"))
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
		Err(Error::new(ErrorKind::Interrupted, "Operation canceled"))
	} else {
		Ok(())
	}
}
