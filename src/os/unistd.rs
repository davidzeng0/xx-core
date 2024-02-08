use super::*;

pub fn close(fd: OwnedFd) -> Result<()> {
	unsafe { syscall_int!(Close, fd)? };

	Ok(())
}
