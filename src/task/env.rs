use std::ops::{Deref, DerefMut};

use crate::pointer::MutPtr;

/// A type that implements [`Global`] declares it
/// as a type which can be infinitely (and simultaneously)
/// mutably borrowed
pub trait Global {
	/// Called when the type implementing Global gets pinned
	unsafe fn pinned(&mut self) {}
}

/// Contains a `Type: Global` and pins its address on the heap
pub struct Boxed<T: Global> {
	data: Box<T>
}

impl<T: Global> Boxed<T> {
	pub fn new(data: T) -> Self {
		let mut data = Box::new(data);

		unsafe {
			data.as_mut().pinned();
		}

		Self { data }
	}

	pub fn get_handle(&mut self) -> Handle<T> {
		self.data.as_mut().into()
	}
}

impl<T: Global> Deref for Boxed<T> {
	type Target = T;

	fn deref(&self) -> &T {
		&self.data
	}
}

impl<T: Global> DerefMut for Boxed<T> {
	fn deref_mut(&mut self) -> &mut T {
		&mut self.data
	}
}

/// A pointer to a [`Global`] type that can be
/// passed around and cloned infinitely
#[derive(PartialEq, Eq)]
pub struct Handle<T: Global + ?Sized> {
	ptr: MutPtr<T>
}

impl<T: Global + Sized> Handle<T> {
	pub unsafe fn new_null() -> Self {
		Self { ptr: MutPtr::<T>::null() }
	}

	pub fn is_null(&self) -> bool {
		self.ptr.is_null()
	}
}

impl<T: Global + ?Sized> Handle<T> {
	pub fn as_raw_ptr(&mut self) -> *const () {
		self.ptr.as_raw_ptr()
	}

	pub fn as_raw_ptr_mut(&mut self) -> *mut () {
		self.ptr.as_raw_ptr_mut()
	}

	pub fn as_ref(&self) -> &T {
		self.ptr.as_ref()
	}

	pub fn as_mut(&mut self) -> &mut T {
		self.ptr.as_mut()
	}
}

impl<T: Global + ?Sized> Clone for Handle<T> {
	#[inline(always)]
	fn clone(&self) -> Self {
		Self { ptr: self.ptr }
	}
}

impl<T: Global + ?Sized> Copy for Handle<T> {}

impl<T: Global + ?Sized> From<MutPtr<T>> for Handle<T> {
	fn from(ptr: MutPtr<T>) -> Self {
		Self { ptr }
	}
}

impl<T: Global + ?Sized> From<&mut T> for Handle<T> {
	fn from(value: &mut T) -> Self {
		Self { ptr: MutPtr::from(value) }
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
		self.as_mut()
	}
}

#[macro_export]
macro_rules! pin_local_mut {
	($var: ident) => {
		let mut $var = $var;

		unsafe {
			$crate::task::env::Global::pinned(&mut $var);
		}
	};
}
