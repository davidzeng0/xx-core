use crate::{async_std::ext::ext_func, coroutines::*, error::Result, xx_core};

#[async_trait_fn]
pub trait Close<Context: AsyncContext> {
	async fn async_close(self) -> Result<()>;
}

pub trait CloseExt<Context: AsyncContext>: Close<Context> + Sized {
	ext_func!(close(self: Self) -> Result<()>);
}

impl<Context: AsyncContext, T: Close<Context>> CloseExt<Context> for T {}
