use std::os::fd::{IntoRawFd, OwnedFd};

use super::syscall::{syscall_int, SyscallNumber::*};
use crate::error::Result;

pub fn close(fd: OwnedFd) -> Result<()> {
	unsafe { syscall_int!(Close, fd.into_raw_fd())? };

	Ok(())
}
