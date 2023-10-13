use std::marker::PhantomData;

use crate::{
	coroutines::{async_fn, env::AsyncContext, runtime::get_context},
	task::env::Handle,
	xx_core
};

pub mod io;

pub trait AsyncIterator<Context: AsyncContext> {
	type Item;

	fn next(&mut self, context: Handle<Context>) -> Option<Self::Item>;
}

pub struct Iterator<Context: AsyncContext, Inner: AsyncIterator<Context>> {
	inner: Inner,
	phantom: PhantomData<Context>
}

#[async_fn]
impl<Context: AsyncContext, Inner: AsyncIterator<Context>> Iterator<Context, Inner> {
	pub fn new(inner: Inner) -> Self {
		Self { inner, phantom: PhantomData }
	}

	pub async fn next(&mut self) -> Option<Inner::Item> {
		self.inner.next(get_context().await)
	}
}
