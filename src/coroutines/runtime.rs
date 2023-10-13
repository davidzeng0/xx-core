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
