use std::ops::{Deref, DerefMut};

/// A type that implements [`Global`] declares it
/// as a type which can be infinitely (and simultaneously)
/// mutably borrowed
pub trait Global {}

/// Contains a `Type: Global` and pins its address on the heap
pub struct Cell<T: Global> {
	data: Box<T>
}

impl<T: Global> Cell<T> {
	pub fn new(data: T) -> Self {
		Cell { data: Box::new(data) }
	}

	pub fn get_shared_ref(&mut self) -> Handle<T> {
		self.data.as_mut().into()
	}
}

impl<T: Global> Deref for Cell<T> {
	type Target = T;

	fn deref(&self) -> &T {
		&self.data
	}
}

impl<T: Global> DerefMut for Cell<T> {
	fn deref_mut(&mut self) -> &mut T {
		&mut self.data
	}
}

/// A pointer to a [`Global`] type that can be
/// passed around and cloned infinitely
#[derive(PartialEq, Eq)]
pub struct Handle<T: Global + ?Sized> {
	value: *mut T
}

impl<T: Global + Sized> Handle<T> {
	pub unsafe fn new_empty() -> Self {
		Self { value: 0 as *mut T }
	}

	pub fn is_null(&self) -> bool {
		self.value == std::ptr::null_mut()
	}
}

impl<T: Global + ?Sized> Handle<T> {
	pub fn as_ptr(&mut self) -> *mut T {
		self.value
	}

	pub fn as_raw_ptr(&mut self) -> *mut () {
		self.value as *mut ()
	}

	pub fn as_ref(&self) -> &T {
		unsafe { &*self.value }
	}

	pub fn get_mut(&mut self) -> &mut T {
		unsafe { &mut *self.value }
	}
}

impl<T: Global + ?Sized> Clone for Handle<T> {
	fn clone(&self) -> Self {
		Self { value: self.value }
	}
}

impl<T: Global + ?Sized> Copy for Handle<T> {}

impl<T: Global + ?Sized> From<*mut T> for Handle<T> {
	fn from(value: *mut T) -> Self {
		Self { value }
	}
}

impl<T: Global + ?Sized> From<&mut T> for Handle<T> {
	fn from(value: &mut T) -> Self {
		Self::from(value as *mut T)
	}
}

impl<T: Global + ?Sized> Deref for Handle<T> {
	type Target = T;

	fn deref(&self) -> &T {
		self.as_ref()
	}
}

impl<T: Global + ?Sized> DerefMut for Handle<T> {
	fn deref_mut(&mut self) -> &mut T {
		self.get_mut()
	}
}
