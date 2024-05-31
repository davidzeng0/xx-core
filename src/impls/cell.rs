use std::{
	cell,
	ops::{Deref, DerefMut}
};

use crate::macros::wrapper_functions;

#[derive(Clone)]
pub struct Cell<T: Copy>(cell::Cell<T>);

impl<T: Copy> Cell<T> {
	wrapper_functions! {
		inner = self.0;

		pub fn into_inner(self) -> T;
	}

	#[must_use]
	pub const fn new(value: T) -> Self {
		Self(cell::Cell::new(value))
	}

	pub fn update<F>(&self, update: F) -> T
	where
		F: FnOnce(T) -> T
	{
		let value = update(self.get());

		self.set(value);

		value
	}
}

impl<T: Copy> Deref for Cell<T> {
	type Target = cell::Cell<T>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: Copy> DerefMut for Cell<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl<T: Copy + PartialEq> PartialEq<T> for Cell<T> {
	fn eq(&self, other: &T) -> bool {
		self.get().eq(other)
	}
}
