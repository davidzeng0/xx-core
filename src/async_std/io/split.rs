use super::*;
use crate::pointer::MutPtr;

/// Splits a stream into a read half and a write half.
/// Implementers must be careful not to violate rust's aliasing rules
pub trait Split: Read + Write {
	fn split(&mut self) -> (ReadRef<'_, Self>, WriteRef<'_, Self>) {
		let mut this = MutPtr::from(self);

		(ReadRef::new(this.as_mut()), WriteRef::new(this.as_mut()))
	}
}
