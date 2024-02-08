use super::*;

define_struct! {
	pub struct IoVec {
		pub base: MutPtr<()>,
		pub len: usize
	}
}

impl From<&[u8]> for IoVec {
	fn from(value: &[u8]) -> Self {
		Self {
			base: Ptr::from(value.as_ptr()).cast_mut().as_unit(),
			len: value.len()
		}
	}
}

impl From<&mut [u8]> for IoVec {
	fn from(value: &mut [u8]) -> Self {
		Self {
			base: MutPtr::from(value.as_mut_ptr()).as_unit(),
			len: value.len()
		}
	}
}
