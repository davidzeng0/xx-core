use std::{
	io::Result,
	os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd}
};

use enumflags2::bitflags;

use super::{
	poll::PollFlag,
	syscall::{syscall_int, SyscallNumber::*}
};
use crate::pointer::MutPtr;

#[bitflags]
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum EPollFlag {
	/// There is data to read.
	In            = PollFlag::In as u32,

	/// There is urgent data to read.
	Priority      = PollFlag::Priority as u32,

	/// Writing now will not block.
	Out           = PollFlag::Out as u32,

	/// Error condition.
	Error         = PollFlag::Error as u32,

	/// Hung up.
	HangUp        = PollFlag::HangUp as u32,

	/// Normal data may be read.
	ReadNorm      = PollFlag::ReadNorm as u32,

	/// Priority data may be read.
	ReadBand      = PollFlag::ReadBand as u32,

	/// Writing now will not block.
	WriteNorm     = PollFlag::WriteNorm as u32,

	/// Priority data may be written.
	WriteBand     = PollFlag::WriteBand as u32,

	Message       = PollFlag::Message as u32,

	RdHangUp      = PollFlag::RdHangUp as u32,

	Exclusive     = 1 << 28,
	WakeUp        = 1 << 29,
	OneShot       = 1 << 30,
	EdgeTriggered = 1 << 31
}

#[repr(u32)]
pub enum CtlOp {
	/// Add a file descriptor to the interface.
	Add = 1,

	/// Remove a file descriptor from the interface.
	Del,

	/// Change file descriptor epoll_event structure.
	Mod
}

#[repr(C, packed)]
pub struct EpollEvent {
	pub events: u32,
	pub data: u64
}

pub fn create(size: i32) -> Result<OwnedFd> {
	let fd = syscall_int!(EpollCreate, size)?;

	Ok(unsafe { OwnedFd::from_raw_fd(fd as i32) })
}

pub fn create1(flags: u32) -> Result<OwnedFd> {
	let fd = syscall_int!(EpollCreate1, flags)?;

	Ok(unsafe { OwnedFd::from_raw_fd(fd as i32) })
}

pub fn ctl(ep: BorrowedFd<'_>, op: CtlOp, fd: i32, event: &mut EpollEvent) -> Result<()> {
	syscall_int!(
		EpollCtl,
		ep.as_raw_fd(),
		op,
		fd,
		MutPtr::from(event).as_raw_int()
	)?;

	Ok(())
}

pub fn wait_raw(
	ep: BorrowedFd<'_>, events: MutPtr<EpollEvent>, count: usize, timeout: i32
) -> Result<u32> {
	let events = syscall_int!(
		EpollWait,
		ep.as_raw_fd(),
		events.as_raw_int(),
		count,
		timeout
	)?;

	Ok(events as u32)
}

pub fn wait(ep: BorrowedFd<'_>, event: &mut [EpollEvent], timeout: i32) -> Result<u32> {
	wait_raw(ep, event.as_mut_ptr().into(), event.len(), timeout)
}
