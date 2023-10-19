use self::ext::ext_func;
use crate::{
	coroutines::{async_trait_fn, env::AsyncContext},
	xx_core
};

pub mod ext;
pub mod io;

pub trait AsyncIterator<Context: AsyncContext> {
	type Item;

	#[async_trait_fn]
	async fn async_next(&mut self) -> Option<Self::Item>;
}

pub trait AsyncIteratorExt<Context: AsyncContext>: AsyncIterator<Context> {
	ext_func!(next(self: &mut Self) -> Option<Self::Item>);
}

impl<Context: AsyncContext, T: AsyncIterator<Context>> AsyncIteratorExt<Context> for T {}
