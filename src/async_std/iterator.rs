#![allow(clippy::module_name_repetitions)]

use super::*;

/// The async equivalent of [`std::iter::Iterator`]
///
/// This trait is object safe
#[asynchronous]
pub trait AsyncIterator {
	type Item;

	/// Returns the next item in the sequence, or `None` if the end has been
	/// reached
	///
	/// See also [`std::iter::Iterator::next`]
	async fn next(&mut self) -> Option<Self::Item>;
}
