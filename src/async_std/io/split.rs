use super::*;
use crate::pointer::MutPtr;

pub trait Split: Read + Write {
	fn split(&mut self) -> (ReadRef<'_, Self>, WriteRef<'_, Self>) {
		let mut this = MutPtr::from(self);

		(ReadRef::new(this.as_mut()), WriteRef::new(this.as_mut()))
	}
}
