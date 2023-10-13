use std::{
	cmp::Ordering,
	ops::{Deref, DerefMut}
};

pub struct Pointer<T: ?Sized, const MUTABLE: bool> {
	ptr: *const T
}

pub type ConstPtr<T> = Pointer<T, false>;
pub type MutPtr<T> = Pointer<T, true>;

impl<T: ?Sized, const MUTABLE: bool> Pointer<T, MUTABLE> {
	pub fn as_ptr(&self) -> *const T {
		self.ptr
	}

	pub fn as_raw_ptr(&self) -> *const () {
		self.ptr as *const _
	}

	pub fn as_raw_int(&self) -> usize {
		self.ptr as *const () as usize
	}

	pub fn cast<T2, const M2: bool>(&self) -> Pointer<T2, M2> {
		Pointer { ptr: self.ptr as *const () as *const _ }
	}

	pub fn is_null(&self) -> bool {
		self.ptr.is_null()
	}

	pub fn into_ptr(self) -> *const T {
		self.as_ptr()
	}

	pub fn into_ref<'a>(self) -> &'a T {
		unsafe { &*self.ptr }
	}
}

impl<T: ?Sized, const MUTABLE: bool> AsRef<T> for Pointer<T, MUTABLE> {
	fn as_ref(&self) -> &T {
		unsafe { &*self.ptr }
	}
}

impl<T, const MUTABLE: bool> Pointer<T, MUTABLE> {
	pub fn null() -> Self {
		Self { ptr: std::ptr::null() }
	}
}

impl<T: ?Sized> MutPtr<T> {
	pub fn as_ptr_mut(&self) -> *mut T {
		self.ptr as *mut _
	}

	pub fn as_raw_ptr_mut(&self) -> *mut () {
		self.ptr as *mut _
	}

	pub fn into_ptr_mut(self) -> *mut T {
		self.as_ptr_mut()
	}

	pub fn into_mut<'a>(self) -> &'a mut T {
		unsafe { &mut *self.as_ptr_mut() }
	}
}

impl<T: ?Sized> AsMut<T> for MutPtr<T> {
	fn as_mut(&mut self) -> &mut T {
		unsafe { &mut *self.as_ptr_mut() }
	}
}

impl<T: ?Sized, const MUTABLE: bool> Clone for Pointer<T, MUTABLE> {
	fn clone(&self) -> Self {
		Self { ptr: self.ptr }
	}
}

impl<T: ?Sized, const MUTABLE: bool> Copy for Pointer<T, MUTABLE> {}

impl<T: ?Sized, const MUTABLE: bool> Deref for Pointer<T, MUTABLE> {
	type Target = T;

	fn deref(&self) -> &T {
		self.as_ref()
	}
}

impl<T: ?Sized> DerefMut for MutPtr<T> {
	fn deref_mut(&mut self) -> &mut T {
		self.as_mut()
	}
}

impl<T: ?Sized> From<MutPtr<T>> for ConstPtr<T> {
	fn from(value: MutPtr<T>) -> Self {
		Self { ptr: value.ptr }
	}
}

impl<T: ?Sized> From<*mut T> for MutPtr<T> {
	fn from(ptr: *mut T) -> Self {
		Self { ptr }
	}
}

impl<T, const MUTABLE: bool> From<usize> for Pointer<T, MUTABLE> {
	fn from(value: usize) -> Self {
		Self { ptr: value as *const _ }
	}
}

impl<T: ?Sized> From<&mut T> for MutPtr<T> {
	fn from(ptr: &mut T) -> Self {
		Self { ptr }
	}
}

impl<T: ?Sized> From<*const T> for ConstPtr<T> {
	fn from(ptr: *const T) -> Self {
		Self { ptr }
	}
}

impl<T: ?Sized> From<&T> for ConstPtr<T> {
	fn from(ptr: &T) -> Self {
		Self { ptr }
	}
}

impl<T: ?Sized, const MUTABLE: bool> PartialEq for Pointer<T, MUTABLE> {
	fn eq(&self, other: &Self) -> bool {
		self.ptr.eq(&other.ptr)
	}
}

impl<T: ?Sized, const MUTABLE: bool> Eq for Pointer<T, MUTABLE> {}

impl<T: ?Sized, const MUTABLE: bool> PartialOrd for Pointer<T, MUTABLE> {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl<T: ?Sized, const MUTABLE: bool> Ord for Pointer<T, MUTABLE> {
	fn cmp(&self, other: &Self) -> Ordering {
		self.ptr.cmp(&other.ptr)
	}
}
