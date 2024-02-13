use std::{
	cell,
	cmp::Ordering,
	fmt::{self, Debug, Formatter, Result},
	ops::{Deref, DerefMut},
	ptr::null_mut,
	rc::Rc
};

use crate::macros::wrapper_functions;

#[repr(transparent)]
pub struct Pointer<T: ?Sized, const MUTABLE: bool> {
	ptr: *mut T
}

pub type Ptr<T> = Pointer<T, false>;
pub type MutPtr<T> = Pointer<T, true>;

impl<T: ?Sized, const MUTABLE: bool> Pointer<T, MUTABLE> {
	wrapper_functions! {
		inner = self.ptr;

		pub fn is_null(self) -> bool;
	}

	pub fn as_ptr(self) -> *const T {
		self.ptr
	}

	pub fn cast<T2>(self) -> Pointer<T2, MUTABLE> {
		Pointer { ptr: self.ptr as *mut () as *mut _ }
	}

	pub fn as_unit(self) -> Pointer<(), MUTABLE> {
		self.cast()
	}

	pub fn int_addr(self) -> usize {
		self.ptr as *const () as usize
	}

	pub fn from_int_addr(value: usize) -> Self
	where
		T: Sized
	{
		Self { ptr: value as *mut _ }
	}

	pub fn offset(self, offset: isize) -> Self
	where
		T: Sized
	{
		Self { ptr: self.ptr.wrapping_offset(offset) }
	}

	pub fn add(self, count: usize) -> Self
	where
		T: Sized
	{
		Self { ptr: self.ptr.wrapping_add(count) }
	}

	pub fn sub(self, count: usize) -> Self
	where
		T: Sized
	{
		Self { ptr: self.ptr.wrapping_sub(count) }
	}

	pub const fn null() -> Self
	where
		T: Sized
	{
		Self { ptr: null_mut() }
	}

	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	pub unsafe fn as_ref<'a>(self) -> &'a T {
		&*self.ptr
	}
}

impl<T, const MUTABLE: bool> Pointer<T, MUTABLE> {
	wrapper_functions! {
		inner = self.ptr;

		pub fn align_offset(self, align: usize) -> usize;
	}
}

impl<T: ?Sized> Ptr<T> {
	pub fn cast_mut(self) -> MutPtr<T> {
		MutPtr { ptr: self.ptr }
	}
}

impl<T: ?Sized> MutPtr<T> {
	wrapper_functions! {
		inner = self.ptr;

		pub unsafe fn drop_in_place(self);
	}

	pub fn cast_const(self) -> Ptr<T> {
		Ptr { ptr: self.ptr }
	}

	pub fn as_mut_ptr(self) -> *mut T {
		self.ptr
	}

	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	pub unsafe fn as_mut<'a>(self) -> &'a mut T {
		&mut *self.ptr
	}
}

impl<T> MutPtr<T> {
	wrapper_functions! {
		inner = self.ptr;

		pub unsafe fn write_bytes(self, val: u8, count: usize);
		pub unsafe fn write(self, value: T);
	}
}

/* derive Clone requires that T: Clone */
impl<T: ?Sized, const MUTABLE: bool> Clone for Pointer<T, MUTABLE> {
	fn clone(&self) -> Self {
		Self { ptr: self.ptr }
	}
}

impl<T: ?Sized, const MUTABLE: bool> Copy for Pointer<T, MUTABLE> {}

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
		Self { ptr: ptr as *const T as *mut T }
	}
}

impl<T: ?Sized> From<&T> for Ptr<T> {
	fn from(ptr: &T) -> Self {
		Self { ptr: ptr as *const T as *mut T }
	}
}

impl<T: ?Sized> From<&mut T> for Ptr<T> {
	fn from(ptr: &mut T) -> Self {
		Self { ptr }
	}
}

impl<T: ?Sized, const MUTABLE: bool> PartialEq for Pointer<T, MUTABLE> {
	fn eq(&self, other: &Self) -> bool {
		std::ptr::eq(self.ptr, other.ptr)
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

impl<T: ?Sized, const MUTABLE: bool> Debug for Pointer<T, MUTABLE> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
		Debug::fmt(&self.ptr, fmt)
	}
}

impl<T: ?Sized, const MUTABLE: bool> fmt::Pointer for Pointer<T, MUTABLE> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
		fmt::Pointer::fmt(&self.ptr, fmt)
	}
}

pub unsafe trait Pin {
	unsafe fn pin(&mut self) {}
}

pub struct Pinned<P> {
	pointer: P
}

impl<P> Pinned<P> {
	pub fn new(pointer: P) -> Self {
		Self { pointer }
	}

	/// Safety: the contract for unpinning P must be satisfied
	pub unsafe fn into_inner(self) -> P {
		self.pointer
	}
}

impl<P: Deref> Deref for Pinned<P> {
	type Target = P::Target;

	fn deref(&self) -> &Self::Target {
		self.pointer.deref()
	}
}

impl<P: DerefMut> DerefMut for Pinned<P> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.pointer.deref_mut()
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

pub trait PinExt: Pin {
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

	fn pin_rc(self) -> Pinned<Rc<Self>>
	where
		Self: Sized
	{
		let mut this = Rc::new(self);

		/* Safety: we are being pinned */
		unsafe { Rc::get_mut(&mut this).unwrap().pin() };

		Pinned::new(this)
	}
}

impl<T: Pin> PinExt for T {}

#[repr(transparent)]
pub struct UnsafeCell<T> {
	data: cell::UnsafeCell<T>
}

impl<T> UnsafeCell<T> {
	pub fn new(data: T) -> Self {
		Self { data: cell::UnsafeCell::new(data) }
	}

	pub fn get(&self) -> MutPtr<T> {
		self.data.get().into()
	}

	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	pub unsafe fn as_ref<'a>(&self) -> &'a T {
		self.get().as_ref()
	}

	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	pub unsafe fn as_mut<'a>(&self) -> &'a mut T {
		self.get().as_mut()
	}

	pub fn into_inner(self) -> T {
		self.data.into_inner()
	}
}

unsafe impl<T: Pin> Pin for UnsafeCell<T> {
	unsafe fn pin(&mut self) {
		self.as_mut().pin()
	}
}
