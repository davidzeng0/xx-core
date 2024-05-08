#![allow(clippy::module_name_repetitions)]

use std::{
	cell,
	cmp::Ordering,
	fmt::{self, Debug, Formatter, Result},
	ops::{Deref, DerefMut},
	ptr::{null_mut, slice_from_raw_parts_mut},
	rc::Rc
};

pub use crate::macros::ptr;
use crate::macros::{seal_trait, wrapper_functions};

#[repr(transparent)]
pub struct Pointer<T: ?Sized, const MUT: bool> {
	ptr: *mut T
}

pub type Ptr<T> = Pointer<T, false>;
pub type MutPtr<T> = Pointer<T, true>;

pub mod internal {
	use super::*;

	seal_trait!();

	impl<T: ?Sized, const MUT: bool> Sealed for Pointer<T, MUT> {}

	pub trait AsPointer: Sealed {
		type Target;

		fn as_pointer(&self) -> Self::Target;
	}

	impl<T: ?Sized> AsPointer for Ptr<T> {
		type Target = *const T;

		fn as_pointer(&self) -> *const T {
			self.ptr
		}
	}

	impl<T: ?Sized> AsPointer for MutPtr<T> {
		type Target = *mut T;

		fn as_pointer(&self) -> *mut T {
			self.ptr
		}
	}

	impl<T: ?Sized> Sealed for UnsafeCell<T> {}

	impl<T: ?Sized> AsPointer for UnsafeCell<T> {
		type Target = *mut T;

		fn as_pointer(&self) -> *mut T {
			self.get().as_pointer()
		}
	}

	pub trait PointerOffset {
		/// # Safety
		/// See [`std::ptr::offset`]
		unsafe fn offset<T, const MUT: bool>(self, pointer: Pointer<T, MUT>) -> Pointer<T, MUT>;
	}

	impl PointerOffset for usize {
		unsafe fn offset<T, const MUT: bool>(
			self, mut pointer: Pointer<T, MUT>
		) -> Pointer<T, MUT> {
			/* Safety: guaranteed by caller */
			pointer.ptr = unsafe { pointer.ptr.add(self) };
			pointer
		}
	}

	impl PointerOffset for isize {
		unsafe fn offset<T, const MUT: bool>(
			self, mut pointer: Pointer<T, MUT>
		) -> Pointer<T, MUT> {
			/* Safety: guaranteed by caller */
			pointer.ptr = unsafe { pointer.ptr.offset(self) };
			pointer
		}
	}
}

impl<T: ?Sized, const MUT: bool> Pointer<T, MUT> {
	wrapper_functions! {
		inner = self.ptr;

		pub fn is_null(self) -> bool;
	}

	#[must_use]
	pub const fn as_ptr(self) -> *const T {
		self.ptr
	}

	#[must_use]
	pub const fn cast<T2>(self) -> Pointer<T2, MUT> {
		Pointer { ptr: self.ptr.cast::<()>().cast() }
	}

	#[must_use]
	pub fn int_addr(self) -> usize {
		self.ptr as *const () as usize
	}

	#[must_use]
	pub const fn from_int_addr(value: usize) -> Self
	where
		T: Sized
	{
		Self { ptr: value as *mut _ }
	}

	/// # Safety
	/// See [`std::ptr::offset`]
	#[must_use]
	#[allow(clippy::impl_trait_in_params)]
	pub unsafe fn offset(self, offset: impl internal::PointerOffset) -> Self
	where
		T: Sized
	{
		/* Safety: guaranteed by caller */
		unsafe { offset.offset(self) }
	}

	#[must_use]
	pub const fn null() -> Self
	where
		T: Sized
	{
		Self { ptr: null_mut() }
	}

	/// # Safety
	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	#[must_use]
	#[allow(clippy::missing_const_for_fn)]
	pub unsafe fn as_ref<'a>(self) -> &'a T {
		/* Safety: guaranteed by caller */
		unsafe { &*self.ptr }
	}
}

impl<T, const MUT: bool> Pointer<T, MUT> {
	wrapper_functions! {
		inner = self.ptr;

		pub fn align_offset(self, align: usize) -> usize;
		pub unsafe fn read(self) -> T;
	}

	/// # Safety
	/// See [`std::ptr::add`]
	#[must_use]
	pub const unsafe fn add(mut self, count: usize) -> Self {
		/* Safety: guaranteed by caller */
		self.ptr = unsafe { self.ptr.add(count) };
		self
	}

	/// # Safety
	/// See [`std::ptr::sub`]
	#[must_use]
	pub const unsafe fn sub(mut self, count: usize) -> Self {
		/* Safety: guaranteed by caller */
		self.ptr = unsafe { self.ptr.sub(count) };
		self
	}
}

impl<T: ?Sized> Ptr<T> {
	#[must_use]
	pub const fn cast_mut(self) -> MutPtr<T> {
		MutPtr { ptr: self.ptr }
	}
}

impl<T: ?Sized> MutPtr<T> {
	wrapper_functions! {
		inner = self.ptr;

		pub unsafe fn drop_in_place(self);
	}

	#[must_use]
	pub const fn cast_const(self) -> Ptr<T> {
		Ptr { ptr: self.ptr }
	}

	#[must_use]
	pub const fn as_mut_ptr(self) -> *mut T {
		self.ptr
	}

	/// # Safety
	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	#[must_use]
	pub unsafe fn as_mut<'a>(self) -> &'a mut T {
		/* Safety: guaranteed by caller */
		unsafe { &mut *self.ptr }
	}
}

impl<T> MutPtr<T> {
	wrapper_functions! {
		inner = self.ptr;

		pub unsafe fn write_bytes(self, val: u8, count: usize);
		pub unsafe fn write(self, value: T);
	}
}

impl<T, const MUT: bool> Pointer<[T], MUT> {
	wrapper_functions! {
		inner = self.ptr;

		#[must_use]
		pub fn len(self) -> usize;

		#[must_use]
		pub fn is_empty(self) -> bool;
	}

	#[must_use]
	pub fn slice_from_raw_parts(data: Pointer<T, MUT>, len: usize) -> Self {
		Self { ptr: slice_from_raw_parts_mut(data.ptr, len) }
	}
}

/* derive Clone requires that T: Clone */
impl<T: ?Sized, const MUT: bool> Clone for Pointer<T, MUT> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: ?Sized, const MUT: bool> Copy for Pointer<T, MUT> {}

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
		Self { ptr: ptr.cast_mut() }
	}
}

impl<T: ?Sized> From<&T> for Ptr<T> {
	fn from(ptr: &T) -> Self {
		#[allow(trivial_casts)]
		Self { ptr: (ptr as *const T).cast_mut() }
	}
}

impl<T: ?Sized, const MUT: bool> PartialEq for Pointer<T, MUT> {
	fn eq(&self, other: &Self) -> bool {
		std::ptr::eq(self.ptr, other.ptr)
	}
}

impl<T: ?Sized, const MUT: bool> Eq for Pointer<T, MUT> {}

impl<T: ?Sized, const MUT: bool> PartialOrd for Pointer<T, MUT> {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl<T: ?Sized, const MUT: bool> Ord for Pointer<T, MUT> {
	#[allow(ambiguous_wide_pointer_comparisons)]
	fn cmp(&self, other: &Self) -> Ordering {
		self.ptr.cmp(&other.ptr)
	}
}

impl<T: Sized, const MUT: bool> Default for Pointer<T, MUT> {
	fn default() -> Self {
		Self::null()
	}
}

impl<T: ?Sized, const MUT: bool> Debug for Pointer<T, MUT> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
		Debug::fmt(&self.ptr, fmt)
	}
}

impl<T: ?Sized, const MUT: bool> fmt::Pointer for Pointer<T, MUT> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
		fmt::Pointer::fmt(&self.ptr, fmt)
	}
}

pub trait Pin {
	/// # Safety
	/// cannot call when already pinned
	unsafe fn pin(&mut self) {}
}

pub struct Pinned<P> {
	pointer: P
}

impl<P> Pinned<P> {
	#[must_use]
	pub const fn new(pointer: P) -> Self {
		Self { pointer }
	}

	/// # Safety
	/// the implementation specific contract for unpinning P must be satisfied
	pub unsafe fn into_inner(self) -> P {
		self.pointer
	}
}

impl<P: Deref> Deref for Pinned<P> {
	type Target = P::Target;

	fn deref(&self) -> &Self::Target {
		&self.pointer
	}
}

impl<P: DerefMut> DerefMut for Pinned<P> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.pointer
	}
}

impl<T> Clone for Pinned<Rc<T>> {
	fn clone(&self) -> Self {
		Self::new(self.pointer.clone())
	}
}

impl<T: Clone> Clone for Pinned<Box<T>> {
	fn clone(&self) -> Self {
		Self::new(self.pointer.clone())
	}
}

seal_trait!(Pin);

pub trait PinExt: PinSealed {
	fn pin_local(&mut self) -> Pinned<&mut Self> {
		let mut pinned = Pinned::new(self);

		/* Safety: we are being pinned */
		unsafe { pinned.pin() };

		pinned
	}

	fn pin_box(self) -> Pinned<Box<Self>>
	where
		Self: Sized
	{
		let mut this = Pinned::new(Box::new(self));

		/* Safety: we are being pinned */
		unsafe { this.pin() };

		this
	}

	#[allow(clippy::unwrap_used)]
	fn pin_rc(self) -> Pinned<Rc<Self>>
	where
		Self: Sized
	{
		let mut rc = Rc::new(self);
		let this = Rc::get_mut(&mut rc).unwrap();

		/* Safety: we are being pinned */
		unsafe { this.pin() };

		Pinned::new(rc)
	}
}

impl<T: PinSealed> PinExt for T {}

#[repr(transparent)]
pub struct UnsafeCell<T: ?Sized> {
	data: cell::UnsafeCell<T>
}

impl<T: ?Sized> UnsafeCell<T> {
	pub const fn new(data: T) -> Self
	where
		T: Sized
	{
		Self { data: cell::UnsafeCell::new(data) }
	}

	pub fn get(&self) -> MutPtr<T> {
		self.data.get().into()
	}

	pub fn get_mut(&mut self) -> &mut T {
		self.data.get_mut()
	}

	/// # Safety
	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	pub unsafe fn as_ref<'a>(&self) -> &'a T {
		/* Safety: guaranteed by caller */
		unsafe { self.get().as_ref() }
	}

	/// # Safety
	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	pub unsafe fn as_mut<'a>(&self) -> &'a mut T {
		/* Safety: guaranteed by caller */
		unsafe { self.get().as_mut() }
	}

	pub fn into_inner(self) -> T
	where
		T: Sized
	{
		self.data.into_inner()
	}
}

impl<T: Pin + ?Sized> Pin for UnsafeCell<T> {
	unsafe fn pin(&mut self) {
		/* Safety: we are being pinned */
		unsafe { self.get_mut().pin() };
	}
}
