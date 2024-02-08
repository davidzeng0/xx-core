use super::*;
use crate::pointer::*;

/// Splits a stream into a read half and a write half.
pub trait Split: Read + Write {
	type Read: Read;
	type Write: Write;

	fn split(&mut self) -> (ReadRef<'_, Self::Read>, WriteRef<'_, Self::Write>);
}

pub unsafe trait SimpleSplit: Read + Write {}

impl<T: SimpleSplit> Split for T {
	type Read = Self;
	type Write = Self;

	fn split(&mut self) -> (ReadRef<'_, Self::Read>, WriteRef<'_, Self::Write>) {
		let this = MutPtr::from(self);

		unsafe { (ReadRef::new(this.as_mut()), WriteRef::new(this.as_mut())) }
	}
}
