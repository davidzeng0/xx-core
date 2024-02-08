use super::*;

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum OpenFlag {
		WriteOnly = 1 << 0,
		ReadWrite = 1 << 1,
		Create    = 1 << 6,
		Excl      = 1 << 7,
		NocTTY    = 1 << 8,
		Truncate  = 1 << 9,
		Append    = 1 << 10,
		NonBlock  = 1 << 11,
		DataSync  = 1 << 12,
		Async     = 1 << 13,
		Direct    = 1 << 14,
		LargeFile = 1 << 15,
		Directory = 1 << 16,
		NoFollow  = 1 << 17,
		NoATime   = 1 << 18,
		CloseExec = 1 << 19,
		Path      = 1 << 21
	}
}

#[allow(non_upper_case_globals)]
impl OpenFlag {
	pub const AccMode: u32 = 0x03;
	pub const ReadOnly: u32 = 0x00;
	pub const Sync: u32 = 0x101000;
	pub const TempFile: u32 = 0x400000;
}
