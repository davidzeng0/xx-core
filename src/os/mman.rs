use std::{
	marker::PhantomData,
	os::fd::{AsRawFd, BorrowedFd}
};

use enumflags2::bitflags;

use super::syscall::*;
use crate::{
	error::Result,
	pointer::{MutPtr, Ptr}
};

#[bitflags]
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MemoryProtection {
	Read      = 1 << 0,
	Write     = 1 << 1,
	Exec      = 1 << 2,
	GrowsDown = 0x01000000,
	GrowsUp   = 0x02000000
}

pub enum MemoryType {
	Shared         = 1,
	Private        = 2,
	SharedValidate = 3
}

#[bitflags]
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MemoryFlag {
	Fixed          = 1 << 4,
	Anonymous      = 1 << 5,
	GrowsDown      = 1 << 8,
	DenyWrite      = 1 << 11,
	Executable     = 1 << 12,
	Locked         = 1 << 13,
	NoReserve      = 1 << 14,
	Populate       = 1 << 15,
	NonBlock       = 1 << 16,
	Stack          = 1 << 17,
	HugeTLB        = 1 << 18,
	Sync           = 1 << 19,
	FixedNoReplace = 1 << 20
}

#[bitflags]
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MemorySyncFlag {
	Async      = 1 << 0,
	Invalidate = 1 << 1,
	Sync       = 1 << 2
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
	Future  = 1 << 1,
	OnFault = 1 << 2
}

#[bitflags]
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MemoryRemapFlag {
	MayMove   = 1 << 0,
	Fixed     = 1 << 1,
	DontUnmap = 1 << 2
}

pub struct MemoryMap<'a> {
	addr: MutPtr<()>,
	length: usize,
	phantom: PhantomData<&'a ()>
}

pub unsafe fn mmap(
	addr: Ptr<()>, length: usize, prot: u32, flags: u32, fd: i32, off: isize
) -> Result<MutPtr<()>> {
	let addr = syscall_pointer!(Mmap, addr.int_addr(), length, prot, flags, fd, off)?;

	Ok(MutPtr::from_int_addr(addr))
}

pub unsafe fn munmap(addr: Ptr<()>, length: usize) -> Result<()> {
	syscall_int!(Munmap, addr.int_addr(), length)?;

	Ok(())
}

pub unsafe fn mprotect(addr: Ptr<()>, length: usize, prot: u32) -> Result<()> {
	syscall_int!(Mprotect, addr.int_addr(), length, prot)?;

	Ok(())
}

pub unsafe fn msync(addr: Ptr<()>, length: usize, flags: u32) -> Result<()> {
	syscall_int!(Msync, addr.int_addr(), length, flags)?;

	Ok(())
}

pub unsafe fn madvise(addr: Ptr<()>, length: usize, advice: u32) -> Result<()> {
	syscall_int!(Msync, addr.int_addr(), length, advice)?;

	Ok(())
}

pub unsafe fn mlock(addr: Ptr<()>, length: usize) -> Result<()> {
	syscall_int!(Mlock, addr.int_addr(), length)?;

	Ok(())
}

pub unsafe fn munlock(addr: Ptr<()>, length: usize) -> Result<()> {
	syscall_int!(Munlock, addr.int_addr(), length)?;

	Ok(())
}

pub unsafe fn mlock_all(flags: u32) -> Result<()> {
	syscall_int!(Mlockall, flags)?;

	Ok(())
}

pub unsafe fn munlock_all() -> Result<()> {
	syscall_int!(Munlockall)?;

	Ok(())
}

pub unsafe fn mremap(
	addr: Ptr<()>, old_length: usize, new_length: usize, flags: u32, new_address: Ptr<()>
) -> Result<MutPtr<()>> {
	let addr = syscall_pointer!(
		Mremap,
		addr.int_addr(),
		old_length,
		new_length,
		flags,
		new_address.int_addr()
	)?;

	Ok(MutPtr::from_int_addr(addr))
}

impl MemoryMap<'_> {
	pub fn new() -> Self {
		Self {
			addr: MutPtr::null(),
			length: 0,
			phantom: PhantomData
		}
	}

	pub fn map<'a>(
		addr: Option<Ptr<()>>, length: usize, prot: u32, flags: u32, fd: Option<BorrowedFd<'_>>,
		off: isize
	) -> Result<MemoryMap<'a>> {
		unsafe {
			let addr = mmap(
				addr.unwrap_or(Ptr::null()),
				length,
				prot,
				flags,
				fd.map(|fd| fd.as_raw_fd()).unwrap_or(-1),
				off
			)?;

			Ok(MemoryMap { addr, length, phantom: PhantomData })
		}
	}

	pub fn addr(&self) -> MutPtr<()> {
		self.addr
	}

	pub fn length(&self) -> usize {
		self.length
	}

	pub fn protect(&mut self, prot: u32) -> Result<()> {
		unsafe { mprotect(self.addr.into(), self.length, prot) }
	}

	pub fn sync(&mut self, flags: u32) -> Result<()> {
		unsafe { msync(self.addr.into(), self.length, flags) }
	}

	pub fn advise(&mut self, advice: u32) -> Result<()> {
		unsafe { madvise(self.addr.into(), self.length, advice) }
	}

	pub fn lock(&mut self) -> Result<()> {
		unsafe { mlock(self.addr.into(), self.length) }
	}

	pub fn unlock(&mut self) -> Result<()> {
		unsafe { munlock(self.addr.into(), self.length) }
	}
}

impl Drop for MemoryMap<'_> {
	fn drop(&mut self) {
		if !self.addr.is_null() {
			unsafe {
				munmap(self.addr.into(), self.length).unwrap();
			}
		}
	}
}
