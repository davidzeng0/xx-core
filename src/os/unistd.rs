use std::{
	io::Result,
	os::fd::{IntoRawFd, OwnedFd}
};

use super::syscall::{syscall_int, SyscallNumber::*};

pub fn close(fd: OwnedFd) -> Result<()> {
	syscall_int!(Close, fd.into_raw_fd())?;

	Ok(())
}
