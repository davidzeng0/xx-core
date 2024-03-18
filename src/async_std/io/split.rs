#![allow(clippy::module_name_repetitions)]

use super::*;
use crate::pointer::*;

/// Splits a stream into a read half and a write half.
pub trait Split: Read + Write {
	type Read: Read;
	type Write: Write;

	fn split(&mut self) -> (ReadRef<'_, Self::Read>, WriteRef<'_, Self::Write>);
}

/// # Safety
/// Implementer must ensure shared mutable access does not occur
pub unsafe trait SimpleSplit: Read + Write {}

impl<T: SimpleSplit> Split for T {
	type Read = Self;
	type Write = Self;

	fn split(&mut self) -> (ReadRef<'_, Self::Read>, WriteRef<'_, Self::Write>) {
		let this = MutPtr::from(self);

		/* Safety: guaranteed by implementer */
		#[allow(clippy::multiple_unsafe_ops_per_block)]
		unsafe {
			(ReadRef::new(this.as_mut()), WriteRef::new(this.as_mut()))
		}
	}
}
