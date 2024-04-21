use core::slice;
use std::{
	io::{IoSlice, IoSliceMut},
	ops::{Deref, DerefMut}
};

use super::*;

pub mod raw {
	use super::*;

	define_struct! {
		pub struct IoVec {
			pub base: MutPtr<()>,
			pub len: usize
		}
	}

	#[repr(transparent)]
	#[derive(Default, Debug)]
	pub struct BorrowedIoVec<'a, const MUT: bool> {
		pub vec: IoVec,
		pub phantom: PhantomData<&'a ()>
	}
}

pub type IoVec<'a> = raw::BorrowedIoVec<'a, false>;
pub type IoVecMut<'a> = raw::BorrowedIoVec<'a, true>;

impl<const MUT: bool> Deref for raw::BorrowedIoVec<'_, MUT> {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		/* Safety: we are borrowing the vec */
		unsafe { slice::from_raw_parts(self.vec.base.as_ptr().cast(), self.vec.len) }
	}
}

impl DerefMut for IoVecMut<'_> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		/* Safety: we are borrowing the vec */
		unsafe { slice::from_raw_parts_mut(self.vec.base.as_mut_ptr().cast(), self.vec.len) }
	}
}

impl<'a> IoVec<'a> {
	#[must_use]
	pub fn from_io_slices<'b>(slices: &'b [IoSlice<'a>]) -> &'b [Self] {
		/* Safety: they are the same */
		#[allow(clippy::transmute_ptr_to_ptr)]
		(unsafe { transmute(slices) })
	}
}

impl<'a> IoVecMut<'a> {
	#[must_use]
	pub fn from_io_slices_mut<'b>(slices: &'b mut [IoSliceMut<'a>]) -> &'b mut [Self] {
		/* Safety: they are the same */
		#[allow(clippy::transmute_ptr_to_ptr)]
		(unsafe { transmute(slices) })
	}
}

impl<'a> From<IoVecMut<'a>> for IoVec<'a> {
	fn from(value: IoVecMut<'a>) -> Self {
		Self { vec: value.vec, phantom: PhantomData }
	}
}

impl<'a> From<&'a [u8]> for IoVec<'a> {
	fn from(value: &'a [u8]) -> Self {
		Self {
			vec: raw::IoVec {
				base: ptr!(value.as_ptr()).cast_mut().cast(),
				len: value.len()
			},
			phantom: PhantomData
		}
	}
}

impl<'a> From<&'a mut [u8]> for IoVecMut<'a> {
	fn from(value: &mut [u8]) -> Self {
		Self {
			vec: raw::IoVec {
				base: ptr!(value.as_mut_ptr()).cast(),
				len: value.len()
			},
			phantom: PhantomData
		}
	}
}
