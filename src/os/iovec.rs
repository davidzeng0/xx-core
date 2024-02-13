use core::slice;
use std::{
	io::{IoSlice, IoSliceMut},
	ops::{Deref, DerefMut}
};

use super::*;

define_struct! {
	pub struct IoVec {
		pub base: MutPtr<()>,
		pub len: usize
	}
}

mod internal {
	use super::*;
	#[repr(transparent)]
	pub struct BorrowedIoVec<'a, const MUTABLE: bool> {
		pub(super) vec: IoVec,
		pub(super) phantom: PhantomData<&'a ()>
	}
}

pub type BorrowedIoVec<'a> = internal::BorrowedIoVec<'a, false>;
pub type BorrowedIoVecMut<'a> = internal::BorrowedIoVec<'a, true>;

impl<'a, const MUTABLE: bool> Deref for internal::BorrowedIoVec<'a, MUTABLE> {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		unsafe { slice::from_raw_parts(self.vec.base.as_ptr().cast(), self.vec.len) }
	}
}

impl<'a> DerefMut for BorrowedIoVecMut<'a> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe { slice::from_raw_parts_mut(self.vec.base.as_mut_ptr().cast(), self.vec.len) }
	}
}

impl<'a> BorrowedIoVec<'a> {
	pub fn from_io_slices<'b>(slices: &'b [IoSlice<'a>]) -> &'b [BorrowedIoVec<'a>] {
		unsafe { transmute(slices) }
	}
}

impl<'a> BorrowedIoVecMut<'a> {
	pub fn from_io_slices_mut<'b>(
		slices: &'b mut [IoSliceMut<'a>]
	) -> &'b mut [BorrowedIoVecMut<'a>] {
		unsafe { transmute(slices) }
	}
}

impl<'a> From<BorrowedIoVecMut<'a>> for BorrowedIoVec<'a> {
	fn from(value: BorrowedIoVecMut<'a>) -> Self {
		Self { vec: value.vec, phantom: PhantomData }
	}
}

impl<'a> From<&'a [u8]> for BorrowedIoVec<'a> {
	fn from(value: &'a [u8]) -> Self {
		Self {
			vec: IoVec {
				base: Ptr::from(value.as_ptr()).cast_mut().as_unit(),
				len: value.len()
			},
			phantom: PhantomData
		}
	}
}

impl<'a> From<&'a mut [u8]> for BorrowedIoVecMut<'a> {
	fn from(value: &mut [u8]) -> Self {
		Self {
			vec: IoVec {
				base: MutPtr::from(value.as_mut_ptr()).as_unit(),
				len: value.len()
			},
			phantom: PhantomData
		}
	}
}
