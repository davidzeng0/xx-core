use std::marker::PhantomData;

use crate::pointer::*;

#[repr(C)]
pub struct IoVec<'a> {
	pub base: MutPtr<()>,
	pub len: usize,

	phantom: PhantomData<&'a ()>
}

impl IoVec<'_> {
	pub fn new() -> Self {
		Self { base: MutPtr::null(), len: 0, phantom: PhantomData }
	}
}

impl From<&[u8]> for IoVec<'_> {
	fn from(value: &[u8]) -> Self {
		Self {
			base: Ptr::from(value.as_ptr()).make_mut().as_unit(),
			len: value.len(),
			phantom: PhantomData
		}
	}
}

impl From<&mut [u8]> for IoVec<'_> {
	fn from(value: &mut [u8]) -> Self {
		Self {
			base: MutPtr::from(value.as_mut_ptr()).as_unit(),
			len: value.len(),
			phantom: PhantomData
		}
	}
}
