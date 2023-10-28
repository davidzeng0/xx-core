use crate::{
	macros::{async_fn, async_trait, async_trait_impl},
	xx_core
};

pub mod io;

#[async_trait]
pub trait AsyncIterator {
	type Item;

	async fn next(&mut self) -> Option<Self::Item>;
}
