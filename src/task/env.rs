use std::ops::{Deref, DerefMut};

use super::*;

/// A type that implements [`Global`] declares it
/// as a type which can be infinitely (and simultaneously)
/// mutably borrowed (limited to same thread mutability)
///
/// If the object can be mutably borrowed across threads,
/// use MutPtr instead
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

	pub unsafe fn into_raw(b: Self) -> MutPtr<T> {
		Box::into_raw(b.data).into()
	}

	pub unsafe fn from_raw(raw: MutPtr<T>) -> Self {
		Self { data: Box::from_raw(raw.as_mut_ptr()) }
	}
}

impl<T: Global> Deref for Boxed<T> {
	type Target = T;

	fn deref(&self) -> &T {
		/* maintain aliasing rules */
		Ptr::from(self.data.as_ref()).as_ref()
	}
}

impl<T: Global> DerefMut for Boxed<T> {
	fn deref_mut(&mut self) -> &mut T {
		self.get_handle().as_mut()
	}
}

/// A pointer to a [`Global`] type that can be
/// passed around and cloned infinitely
#[derive(PartialEq, Eq)]
#[repr(transparent)]
pub struct Handle<T: Global + ?Sized> {
	ptr: MutPtr<T>
}

impl<T: Global + Sized> Handle<T> {
	pub unsafe fn null() -> Self {
		Self { ptr: MutPtr::<T>::null() }
	}

	pub fn is_null(&self) -> bool {
		self.ptr.is_null()
	}
}

impl<T: Global + ?Sized> Handle<T> {
	pub fn as_ref<'a>(&self) -> &'a T {
		self.ptr.as_ref()
	}

	pub fn as_mut<'a>(&mut self) -> &'a mut T {
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
			$crate::task::Global::pinned(&mut $var);
		}
	};
}

pub(crate) use pin_local_mut;
