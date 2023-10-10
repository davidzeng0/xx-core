use enumflags2::bitflags;

#[bitflags]
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ResolveFlag {
	NoExternalDevice = 1 << 0,
	NoMagicLinks     = 1 << 1,
	NoSymlinks       = 1 << 2,
	Beneath          = 1 << 3,
	InRoot           = 1 << 4,
	Cached           = 1 << 5
}

#[repr(C)]
pub struct OpenHow {
	pub flags: u64,
	pub mode: u64,
	pub resolve: u64
}
