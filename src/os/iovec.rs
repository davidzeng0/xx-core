use crate::pointer::MutPtr;

#[repr(C)]
pub struct IoVec {
	pub base: MutPtr<()>,
	pub len: usize
}
