use super::*;

#[asynchronous]
pub trait AsyncIterator {
	type Item;

	async fn next(&mut self) -> Option<Self::Item>;
}
