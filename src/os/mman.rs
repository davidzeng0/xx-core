use super::syscall::{syscall_int, syscall_pointer, SyscallNumber::*};
use enumflags2::bitflags;
use std::{
	io::Result,
	os::fd::{AsRawFd, BorrowedFd}
};

#[bitflags]
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MemoryProtection {
	Read = 1 << 0,
	Write = 1 << 1,
	Exec = 1 << 2,
	GrowsDown = 0x01000000,
	GrowsUp = 0x02000000
}

pub enum MemoryType {
	Shared = 1,
	Private = 2,
	SharedValidate = 3
}

#[bitflags]
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MemoryFlag {
	Fixed = 1 << 4,
	Anonymous = 1 << 5,
	GrowsDown = 1 << 8,
	DenyWrite = 1 << 11,
	Executable = 1 << 12,
	Locked = 1 << 13,
	NoReserve = 1 << 14,
	Populate = 1 << 15,
	NonBlock = 1 << 16,
	Stack = 1 << 17,
	HugeTLB = 1 << 18,
	Sync = 1 << 19,
	FixedNoReplace = 1 << 20
}

#[bitflags]
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MemorySyncFlag {
	Async = 1 << 0,
	Invalidate = 1 << 1,
	Sync = 1 << 2
}

pub enum MemoryAdvice {
	Normal,
	Random,
	Sequential,
	WillNeed,
	DontNeed,
	Free = 8,
	Remove,
	DontFork,
	DoFork,
	Mergeable,
	Unmergeable,
	HugePage,
	NoHugePage,
	DontDump,
	DoDump,
	WipeOnFork,
	KeepOnFork,
	Cold,
	PageOut,
	PopulateRead,
	PopulateWrite,
	HwPoison
}

#[bitflags]
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MemoryLockFlag {
	Current = 1 << 0,
	Future = 1 << 1,
	OnFault = 1 << 2
}

#[bitflags]
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MemoryRemapFlag {
	MayMove = 1 << 0,
	Fixed = 1 << 1,
	DontUnmap = 1 << 2
}

pub struct MemoryMap {
	pub addr: usize,
	pub length: usize
}

pub fn mmap(addr: usize, length: usize, prot: u32, flags: u32, fd: i32, off: isize) -> Result<usize> {
	syscall_pointer!(Mmap, addr, length, prot, flags, fd, off)
}

pub fn map_memory(addr: usize, length: usize, prot: u32, flags: u32, fd: Option<BorrowedFd<'_>>, off: isize) -> Result<MemoryMap> {
	let addr = mmap(addr, length, prot, flags, fd.map(|fd| fd.as_raw_fd()).unwrap_or(-1), off)?;

	Ok(MemoryMap { addr, length })
}

pub fn munmap(addr: usize, length: usize) -> Result<()> {
	syscall_int!(Munmap, addr, length)?;

	Ok(())
}

pub fn mprotect(addr: usize, length: usize, prot: u32) -> Result<()> {
	syscall_int!(Mprotect, addr, length, prot)?;

	Ok(())
}

pub fn msync(addr: usize, length: usize, flags: u32) -> Result<()> {
	syscall_int!(Msync, addr, length, flags)?;

	Ok(())
}

pub fn madvise(addr: usize, length: usize, advice: u32) -> Result<()> {
	syscall_int!(Msync, addr, length, advice)?;

	Ok(())
}

pub fn mlock(addr: usize, length: usize) -> Result<()> {
	syscall_int!(Mlock, addr, length)?;

	Ok(())
}

pub fn munlock(addr: usize, length: usize) -> Result<()> {
	syscall_int!(Munlock, addr, length)?;

	Ok(())
}

pub fn mlock_all(flags: u32) -> Result<()> {
	syscall_int!(Mlockall, flags)?;

	Ok(())
}

pub fn munlock_all() -> Result<()> {
	syscall_int!(Munlockall)?;

	Ok(())
}

pub fn mremap(addr: usize, old_length: usize, new_length: usize, flags: u32, new_address: usize) -> Result<usize> {
	syscall_pointer!(Mremap, addr, old_length, new_length, flags, new_address)
}

impl MemoryMap {
	pub fn new() -> MemoryMap {
		MemoryMap { addr: 0, length: 0 }
	}

	pub fn protect(&mut self, prot: u32) -> Result<()> {
		mprotect(self.addr, self.length, prot)
	}

	pub fn sync(&mut self, flags: u32) -> Result<()> {
		msync(self.addr, self.length, flags)
	}

	pub fn advise(&mut self, advice: u32) -> Result<()> {
		madvise(self.addr, self.length, advice)
	}

	pub fn lock(&mut self) -> Result<()> {
		mlock(self.addr, self.length)
	}

	pub fn unlock(&mut self) -> Result<()> {
		munlock(self.addr, self.length)
	}
}

impl Drop for MemoryMap {
	fn drop(&mut self) {
		if self.addr != 0 {
			munmap(self.addr, self.length).unwrap();
		}
	}
}
