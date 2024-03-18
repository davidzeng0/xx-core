use super::*;

#[syscall_define(Close)]
pub fn close(fd: OwnedFd) -> Result<()>;
