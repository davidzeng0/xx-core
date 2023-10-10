use enumflags2::bitflags;

#[bitflags]
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PollFlag {
	/// There is data to read.
	In        = 1 << 0,

	/// There is urgent data to read.
	Priority  = 1 << 1,

	/// Writing now will not block.
	Out       = 1 << 2,

	/// Error condition.
	Error     = 1 << 3,

	/// Hung up.
	HangUp    = 1 << 4,

	/// Invalid polling request.
	Invalid   = 1 << 5,

	/// Normal data may be read.
	ReadNorm  = 1 << 6,

	/// Priority data may be read.
	ReadBand  = 1 << 7,

	/// Writing now will not block.
	WriteNorm = 1 << 8,

	/// Priority data may be written.
	WriteBand = 1 << 9,

	/// Extensions for Linux
	Message   = 1 << 10,
	Remove    = 1 << 12,
	RdHangUp  = 1 << 13
}
