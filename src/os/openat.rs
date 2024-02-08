use super::*;

define_enum! {
	#[repr(i32)]
	pub enum OpenAt {
		CurrentWorkingDirectory = -100
	}
}
