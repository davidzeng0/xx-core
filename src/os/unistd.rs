use std::{
	io::Result,
	os::fd::{AsRawFd, OwnedFd}
};

use super::syscall::{syscall_int, SyscallNumber::*};

pub fn close(fd: OwnedFd) -> Result<()> {
	syscall_int!(Close, fd.as_raw_fd())?;

	Ok(())
}
