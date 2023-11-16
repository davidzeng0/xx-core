use enumflags2::bitflags;

use super::syscall::*;
use crate::{error::Result, pointer::MutPtr};

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

#[repr(C)]
pub struct PollFd {
	pub fd: i32,
	pub events: u16,
	pub returned_events: u16
}

pub fn poll(fds: &mut [PollFd], timeout: i32) -> Result<u32> {
	let events = syscall_int!(
		Poll,
		MutPtr::from(fds.as_mut_ptr()).int_addr(),
		fds.len(),
		timeout
	)?;

	Ok(events as u32)
}
