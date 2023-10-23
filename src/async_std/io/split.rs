use super::{Read, ReadRef, Write, WriteRef};
use crate::{coroutines::*, pointer::MutPtr};

pub trait Split<Context: AsyncContext>: Read<Context> + Write<Context> {
	fn split(&mut self) -> (ReadRef<'_, Context, Self>, WriteRef<'_, Context, Self>) {
		let mut this = MutPtr::from(self);

		(ReadRef::new(this.as_mut()), WriteRef::new(this.as_mut()))
	}
}
