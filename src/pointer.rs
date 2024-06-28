#![allow(clippy::module_name_repetitions)]

use std::fmt::{self, Debug, Formatter, Result};
use std::mem::transmute;
use std::num::NonZeroUsize;
use std::ops::{Deref, DerefMut};
use std::ptr::{self, null_mut, slice_from_raw_parts_mut};
use std::rc::Rc;
use std::sync::{atomic, Arc};
use std::{cell, cmp, result};

pub use crate::macros::ptr;
use crate::macros::{assert_unsafe_precondition, seal_trait, wrapper_functions};

pub mod internal {
	use super::*;

	seal_trait!();

	pub trait AsPointer: Sealed {
		type Target;

		fn as_pointer(&self) -> Self::Target;
	}

	impl<T: ?Sized, const MUT: bool> Sealed for Pointer<T, MUT> {}

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

	impl<T: ?Sized, const MUT: bool> Sealed for NonNullPtr<T, MUT> {}

	impl<T: ?Sized> AsPointer for NonNull<T> {
		type Target = *const T;

		fn as_pointer(&self) -> *const T {
			self.as_ptr()
		}
	}

	impl<T: ?Sized> AsPointer for MutNonNull<T> {
		type Target = *mut T;

		fn as_pointer(&self) -> *mut T {
			self.as_mut_ptr()
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

	pub trait PointerIndex<const MUT: bool, Idx> {
		type Output: ?Sized;

		/// # Safety
		/// See [`std::ptr::offset`]
		unsafe fn index(self, index: Idx) -> Pointer<Self::Output, MUT>;
	}

	impl<const MUT: bool, T> PointerIndex<MUT, usize> for Pointer<[T], MUT> {
		type Output = T;

		unsafe fn index(self, index: usize) -> Pointer<T, MUT> {
			/* Safety: guaranteed by caller */
			unsafe { self.cast().add(index) }
		}
	}
}

#[repr(transparent)]
pub struct Pointer<T: ?Sized, const MUT: bool> {
	ptr: *mut T
}

pub type Ptr<T> = Pointer<T, false>;
pub type MutPtr<T> = Pointer<T, true>;

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
		Pointer { ptr: self.ptr.cast() }
	}

	#[must_use]
	pub fn addr(self) -> usize {
		self.as_ptr().cast::<()>() as usize
	}

	/// # Safety
	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	#[must_use]
	#[allow(clippy::missing_const_for_fn)]
	pub unsafe fn as_ref<'a>(self) -> &'a T {
		/* Safety: guaranteed by caller */
		unsafe { assert_unsafe_precondition!(!self.is_null()) };

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

	/// # Safety
	/// See [`std::ptr::offset`]
	#[must_use]
	#[allow(clippy::impl_trait_in_params)]
	pub unsafe fn offset(self, offset: impl internal::PointerOffset) -> Self {
		/* Safety: guaranteed by caller */
		unsafe { offset.offset(self) }
	}

	#[must_use]
	pub const fn from_addr(value: usize) -> Self {
		Self { ptr: value as *mut _ }
	}

	#[must_use]
	pub const fn null() -> Self {
		Self { ptr: null_mut() }
	}

	/// # Safety
	/// `self` must not be null
	#[must_use]
	pub const unsafe fn cast_nonnull(self) -> NonNullPtr<T, MUT> {
		/* Safety: guaranteed by caller */
		unsafe { NonNullPtr::new_unchecked(self) }
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
		unsafe { assert_unsafe_precondition!(!self.is_null()) };

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

pub struct PointerIterator<T, const MUT: bool> {
	start: Pointer<T, MUT>,
	end: Pointer<T, MUT>
}

impl<T, const MUT: bool> Iterator for PointerIterator<T, MUT> {
	type Item = Pointer<T, MUT>;

	fn next(&mut self) -> Option<Self::Item> {
		let cur = self.start;

		if cur < self.end {
			/* Safety: start < end */
			unsafe { self.start = self.start.add(1) };

			Some(cur)
		} else {
			None
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		#[allow(clippy::cast_sign_loss)]
		/* Safety: start < end */
		let len = unsafe { self.end.as_ptr().offset_from(self.start.as_ptr()) } as usize;

		(len, Some(len))
	}
}

impl<T, const MUT: bool> DoubleEndedIterator for PointerIterator<T, MUT> {
	fn next_back(&mut self) -> Option<Self::Item> {
		if self.end > self.start {
			/* Safety: start < end */
			unsafe { self.end = self.end.sub(1) };

			Some(self.end)
		} else {
			None
		}
	}
}

impl<T, const MUT: bool> ExactSizeIterator for PointerIterator<T, MUT> {}

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

	/// # Safety
	/// The slice must be one contiguous memory segment
	#[must_use]
	pub unsafe fn into_iter(self) -> PointerIterator<T, MUT> {
		let start = self.cast::<T>();

		/* Safety: guaranteed by caller */
		let end = unsafe { start.add(self.len()) };

		PointerIterator { start, end }
	}
}

/* derive Clone requires that T: Clone */
impl<T: ?Sized, const MUT: bool> Clone for Pointer<T, MUT> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: ?Sized, const MUT: bool> Copy for Pointer<T, MUT> {}

impl<T: ?Sized, const MUT: bool> From<NonNullPtr<T, MUT>> for Pointer<T, MUT> {
	fn from(value: NonNullPtr<T, MUT>) -> Self {
		value.as_pointer()
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

impl<T: ?Sized, const MUT: bool> PartialEq for Pointer<T, MUT> {
	fn eq(&self, other: &Self) -> bool {
		ptr::eq(self.ptr, other.ptr)
	}
}

impl<T: ?Sized, const MUT: bool> Eq for Pointer<T, MUT> {}

impl<T: ?Sized, const MUT: bool> PartialOrd for Pointer<T, MUT> {
	fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl<T: ?Sized, const MUT: bool> Ord for Pointer<T, MUT> {
	#[allow(ambiguous_wide_pointer_comparisons)]
	fn cmp(&self, other: &Self) -> cmp::Ordering {
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

#[repr(transparent)]
pub struct NonNullPtr<T: ?Sized, const MUT: bool> {
	ptr: ptr::NonNull<T>
}

pub type NonNull<T> = NonNullPtr<T, false>;
pub type MutNonNull<T> = NonNullPtr<T, true>;

impl<T: ?Sized, const MUT: bool> NonNullPtr<T, MUT> {
	/// # Safety
	/// `ptr` must not be null
	#[must_use]
	pub const unsafe fn new_unchecked(ptr: Pointer<T, MUT>) -> Self {
		/* Safety: guaranteed by caller */
		let ptr = unsafe { ptr::NonNull::new_unchecked(ptr.ptr) };

		Self { ptr }
	}

	#[must_use]
	pub fn new(ptr: Pointer<T, MUT>) -> Option<Self> {
		ptr::NonNull::new(ptr.ptr).map(|ptr| Self { ptr })
	}

	#[must_use]
	pub const fn as_pointer(self) -> Pointer<T, MUT> {
		Pointer { ptr: self.ptr.as_ptr() }
	}

	#[must_use]
	pub const fn as_ptr(self) -> *const T {
		self.ptr.as_ptr()
	}

	#[must_use]
	pub const fn cast<T2>(self) -> NonNullPtr<T2, MUT> {
		NonNullPtr { ptr: self.ptr.cast() }
	}

	#[must_use]
	pub fn addr(self) -> usize {
		self.as_pointer().addr()
	}

	/// # Safety
	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	#[must_use]
	#[allow(clippy::missing_const_for_fn)]
	pub unsafe fn as_ref<'a>(self) -> &'a T {
		/* Safety: guaranteed by caller */
		unsafe { self.as_pointer().as_ref() }
	}
}

impl<T, const MUT: bool> NonNullPtr<T, MUT> {
	wrapper_functions! {
		inner = self.as_pointer();

		pub fn align_offset(self, align: usize) -> usize;
		pub unsafe fn read(self) -> T;
	}

	#[must_use]
	pub const fn dangling() -> Self {
		Self { ptr: ptr::NonNull::dangling() }
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

	/// # Safety
	/// See [`std::ptr::offset`]
	#[must_use]
	#[allow(clippy::impl_trait_in_params)]
	pub unsafe fn offset(self, offset: impl internal::PointerOffset) -> Self {
		/* Safety: guaranteed by caller */
		#[allow(clippy::multiple_unsafe_ops_per_block)]
		unsafe {
			Self::new_unchecked(offset.offset(self.as_pointer()))
		}
	}

	/// # Safety
	/// `value` must be non zero
	#[must_use]
	pub const unsafe fn from_addr_unchecked(value: usize) -> Self {
		/* Safety: guaranteed by caller */
		unsafe { Self::new_unchecked(Pointer::from_addr(value)) }
	}

	#[must_use]
	pub const fn from_addr(value: NonZeroUsize) -> Self {
		/* Safety: value is non zero */
		unsafe { Self::from_addr_unchecked(value.get()) }
	}
}

impl<T: ?Sized> NonNull<T> {
	#[must_use]
	pub const fn cast_mut(self) -> MutNonNull<T> {
		MutNonNull { ptr: self.ptr }
	}
}

impl<T: ?Sized> MutNonNull<T> {
	wrapper_functions! {
		inner = self.as_pointer();

		pub unsafe fn drop_in_place(self);
	}

	#[must_use]
	pub const fn cast_const(self) -> NonNull<T> {
		NonNull { ptr: self.ptr }
	}

	#[must_use]
	pub const fn as_mut_ptr(self) -> *mut T {
		self.ptr.as_ptr()
	}

	/// # Safety
	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	#[must_use]
	pub unsafe fn as_mut<'a>(self) -> &'a mut T {
		/* Safety: guaranteed by caller */
		unsafe { self.as_pointer().as_mut() }
	}

	#[must_use]
	pub fn from_box(ptr: Box<T>) -> Self {
		/* Safety: Box::into_raw is always non null */
		unsafe { Self::new_unchecked(Box::into_raw(ptr).into()) }
	}

	/// # Safety
	/// See [`Box::from_raw`]
	#[must_use]
	pub unsafe fn into_box(self) -> Box<T> {
		/* Safety: Box::into_raw is always non null */
		unsafe { Box::from_raw(self.as_mut_ptr()) }
	}
}

impl<T> MutNonNull<T> {
	wrapper_functions! {
		inner = self.as_pointer();

		pub unsafe fn write_bytes(self, val: u8, count: usize);
		pub unsafe fn write(self, value: T);
	}
}

impl<T, const MUT: bool> NonNullPtr<[T], MUT> {
	wrapper_functions! {
		inner = self.as_pointer();

		#[must_use]
		pub fn len(self) -> usize;

		#[must_use]
		pub fn is_empty(self) -> bool;

		#[must_use]
		pub unsafe fn into_iter(self) -> PointerIterator<T, MUT>;
	}

	#[must_use]
	pub fn slice_from_raw_parts(data: NonNullPtr<T, MUT>, len: usize) -> Self {
		Self {
			ptr: ptr::NonNull::slice_from_raw_parts(data.ptr, len)
		}
	}
}

impl<T: ?Sized, const MUT: bool> Clone for NonNullPtr<T, MUT> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: ?Sized, const MUT: bool> Copy for NonNullPtr<T, MUT> {}

impl<T: ?Sized> From<&T> for NonNull<T> {
	fn from(ptr: &T) -> Self {
		#[allow(trivial_casts)]
		Self { ptr: ptr.into() }
	}
}

impl<T: ?Sized> From<&mut T> for MutNonNull<T> {
	fn from(ptr: &mut T) -> Self {
		Self { ptr: ptr.into() }
	}
}

impl<T: ?Sized, const MUT: bool> PartialEq for NonNullPtr<T, MUT> {
	fn eq(&self, other: &Self) -> bool {
		self.as_pointer().eq(&other.as_pointer())
	}
}

impl<T: ?Sized, const MUT: bool> Eq for NonNullPtr<T, MUT> {}

impl<T: ?Sized, const MUT: bool> PartialOrd for NonNullPtr<T, MUT> {
	fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl<T: ?Sized, const MUT: bool> Ord for NonNullPtr<T, MUT> {
	#[allow(ambiguous_wide_pointer_comparisons)]
	fn cmp(&self, other: &Self) -> cmp::Ordering {
		self.ptr.cmp(&other.ptr)
	}
}

impl<T: Sized, const MUT: bool> Default for NonNullPtr<T, MUT> {
	fn default() -> Self {
		Self::dangling()
	}
}

impl<T: ?Sized, const MUT: bool> Debug for NonNullPtr<T, MUT> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
		Debug::fmt(&self.ptr, fmt)
	}
}

impl<T: ?Sized, const MUT: bool> fmt::Pointer for NonNullPtr<T, MUT> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
		fmt::Pointer::fmt(&self.ptr, fmt)
	}
}

pub trait Pin {
	/// # Safety
	/// cannot call when already pinned
	unsafe fn pin(&mut self) {}
}

pub struct Pinned<P>(P);

impl<P> Pinned<P> {
	#[must_use]
	pub const fn new(pointer: P) -> Self {
		Self(pointer)
	}

	/// # Safety
	/// the implementation specific contract for unpinning P must be satisfied
	pub unsafe fn into_inner(self) -> P {
		self.0
	}
}

impl<P: Deref> Deref for Pinned<P> {
	type Target = P::Target;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<P: DerefMut> DerefMut for Pinned<P> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl<P: Clone> Clone for Pinned<P> {
	fn clone(&self) -> Self {
		Self::new(self.0.clone())
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

	#[allow(clippy::unwrap_used)]
	fn pin_arc(self) -> Pinned<Arc<Self>>
	where
		Self: Sized
	{
		let mut arc = Arc::new(self);
		let this = Arc::get_mut(&mut arc).unwrap();

		/* Safety: we are being pinned */
		unsafe { this.pin() };

		Pinned::new(arc)
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

pub struct AtomicPointer<T, const MUT: bool> {
	ptr: atomic::AtomicPtr<T>
}

pub type AtomicMutPtr<T> = AtomicPointer<T, true>;
pub type AtomicPtr<T> = AtomicPointer<T, false>;

impl<T, const MUT: bool> AtomicPointer<T, MUT> {
	#[must_use]
	pub const fn new(value: Pointer<T, MUT>) -> Self {
		Self { ptr: atomic::AtomicPtr::new(value.ptr) }
	}

	pub fn get_mut(&mut self) -> &mut Pointer<T, MUT> {
		/* Safety: repr transparent */
		#[allow(clippy::transmute_ptr_to_ptr)]
		unsafe {
			transmute(self.ptr.get_mut())
		}
	}

	pub const fn into_inner(self) -> Pointer<T, MUT> {
		Pointer { ptr: self.ptr.into_inner() }
	}

	pub fn load(&self, order: atomic::Ordering) -> Pointer<T, MUT> {
		Pointer { ptr: self.ptr.load(order) }
	}

	pub fn store(&self, value: Pointer<T, MUT>, order: atomic::Ordering) {
		self.ptr.store(value.ptr, order);
	}

	pub fn swap(&self, value: Pointer<T, MUT>, order: atomic::Ordering) -> Pointer<T, MUT> {
		Pointer { ptr: self.ptr.swap(value.ptr, order) }
	}

	pub fn compare_exchange(
		&self, current: Pointer<T, MUT>, new: Pointer<T, MUT>, success: atomic::Ordering,
		failure: atomic::Ordering
	) -> result::Result<Pointer<T, MUT>, Pointer<T, MUT>> {
		match self
			.ptr
			.compare_exchange(current.ptr, new.ptr, success, failure)
		{
			Ok(ptr) => Ok(Pointer { ptr }),
			Err(ptr) => Err(Pointer { ptr })
		}
	}

	pub fn compare_exchange_weak(
		&self, current: Pointer<T, MUT>, new: Pointer<T, MUT>, success: atomic::Ordering,
		failure: atomic::Ordering
	) -> result::Result<Pointer<T, MUT>, Pointer<T, MUT>> {
		match self
			.ptr
			.compare_exchange_weak(current.ptr, new.ptr, success, failure)
		{
			Ok(ptr) => Ok(Pointer { ptr }),
			Err(ptr) => Err(Pointer { ptr })
		}
	}

	pub fn fetch_update<F>(
		&self, set_order: atomic::Ordering, fetch_order: atomic::Ordering, mut update: F
	) -> result::Result<Pointer<T, MUT>, Pointer<T, MUT>>
	where
		F: FnMut(Pointer<T, MUT>) -> Option<Pointer<T, MUT>>
	{
		match self.ptr.fetch_update(set_order, fetch_order, |ptr| {
			update(Pointer { ptr }).map(|ptr| ptr.ptr)
		}) {
			Ok(ptr) => Ok(Pointer { ptr }),
			Err(ptr) => Err(Pointer { ptr })
		}
	}
}
