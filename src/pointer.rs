use std::{
	cmp::Ordering,
	mem::MaybeUninit,
	ops::{Deref, DerefMut},
	ptr::null
};

#[repr(transparent)]
pub struct Pointer<T: ?Sized, const MUTABLE: bool> {
	ptr: *const T
}

pub type Ptr<T> = Pointer<T, false>;
pub type MutPtr<T> = Pointer<T, true>;

impl<T: ?Sized, const MUTABLE: bool> Pointer<T, MUTABLE> {
	pub fn as_ptr(&self) -> *const T {
		self.ptr
	}

	pub fn cast<T2>(&self) -> Pointer<T2, MUTABLE> {
		Pointer { ptr: self.ptr as *const () as *const _ }
	}

	pub fn as_unit(&self) -> Pointer<(), MUTABLE> {
		self.cast()
	}

	pub fn as_uninit(self) -> Pointer<MaybeUninit<T>, MUTABLE>
	where
		T: Sized
	{
		self.cast()
	}

	pub fn int_addr(&self) -> usize {
		self.ptr as *const () as usize
	}

	pub fn from_int_addr(value: usize) -> Self
	where
		T: Sized
	{
		Self { ptr: value as *const _ }
	}

	pub fn is_null(&self) -> bool {
		self.ptr.is_null()
	}

	/// Aliasing rules must be enforced. See std::ptr::as_ref
	pub fn as_ref<'a>(&self) -> &'a T {
		unsafe { &*self.ptr }
	}

	pub fn wrapping_offset(&self, offset: isize) -> Self
	where
		T: Sized
	{
		Self { ptr: self.ptr.wrapping_offset(offset) }
	}

	pub fn wrapping_add(&self, count: usize) -> Self
	where
		T: Sized
	{
		Self { ptr: self.ptr.wrapping_add(count) }
	}

	pub fn wrapping_sub(&self, count: usize) -> Self
	where
		T: Sized
	{
		Self { ptr: self.ptr.wrapping_sub(count) }
	}

	pub fn null() -> Self
	where
		T: Sized
	{
		Self { ptr: null() }
	}
}

impl<T: ?Sized> Ptr<T> {
	pub fn make_mut(self) -> MutPtr<T> {
		MutPtr { ptr: self.ptr }
	}
}

impl<T: ?Sized> MutPtr<T> {
	pub fn as_mut_ptr(&self) -> *mut T {
		self.ptr as *mut _
	}

	/// Aliasing rules must be enforced. See std::ptr::as_mut
	pub fn as_mut<'a>(&mut self) -> &'a mut T {
		unsafe { &mut *MutPtr::as_mut_ptr(self) }
	}
}

/* derive Clone requires that T: Clone */
impl<T: ?Sized, const MUTABLE: bool> Clone for Pointer<T, MUTABLE> {
	fn clone(&self) -> Self {
		Self { ptr: self.ptr }
	}
}

impl<T: ?Sized, const MUTABLE: bool> Copy for Pointer<T, MUTABLE> {}

impl<T: ?Sized, const MUTABLE: bool> Deref for Pointer<T, MUTABLE> {
	type Target = T;

	/// Aliasing rules must be enforced. See as_ref
	fn deref(&self) -> &T {
		self.as_ref()
	}
}

impl<T: ?Sized> DerefMut for MutPtr<T> {
	/// Aliasing rules must be enforced. See as_mut
	fn deref_mut(&mut self) -> &mut T {
		self.as_mut()
	}
}

impl<T: ?Sized> From<MutPtr<T>> for Ptr<T> {
	fn from(value: MutPtr<T>) -> Self {
		Self { ptr: value.ptr }
	}
}

impl<T: ?Sized> From<*mut T> for MutPtr<T> {
	fn from(ptr: *mut T) -> Self {
		Self { ptr }
	}
}

impl<T: ?Sized> From<&mut T> for MutPtr<T> {
	fn from(ptr: &mut T) -> Self {
		Self { ptr }
	}
}

impl<T: ?Sized> From<*const T> for Ptr<T> {
	fn from(ptr: *const T) -> Self {
		Self { ptr }
	}
}

impl<T: ?Sized> From<&T> for Ptr<T> {
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
