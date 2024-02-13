use super::*;

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum MemoryProtection {
		Read      = 1 << 0,
		Write     = 1 << 1,
		Exec      = 1 << 2,
		GrowsDown = 0x01000000,
		GrowsUp   = 0x02000000
	}
}

define_enum! {
	#[repr(u32)]
	pub enum MemoryType {
		Shared         = 1,
		Private        = 2,
		SharedValidate = 3
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
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
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum MemorySyncFlag {
		Async      = 1 << 0,
		Invalidate = 1 << 1,
		Sync       = 1 << 2
	}
}

define_enum! {
	#[repr(u32)]
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
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum MemoryLockFlag {
		Current = 1 << 0,
		Future  = 1 << 1,
		OnFault = 1 << 2
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum MemoryRemapFlag {
		MayMove   = 1 << 0,
		Fixed     = 1 << 1,
		DontUnmap = 1 << 2
	}
}

pub struct MemoryMap<'a> {
	addr: MutPtr<()>,
	length: usize,
	phantom: PhantomData<&'a ()>
}

pub unsafe fn mmap(
	addr: Ptr<()>, length: usize, prot: u32, flags: u32, fd: i32, off: isize
) -> Result<MutPtr<()>> {
	let addr = syscall_pointer!(Mmap, addr, length, prot, flags, fd, off)?;

	Ok(MutPtr::from_int_addr(addr))
}

pub unsafe fn munmap(addr: Ptr<()>, length: usize) -> Result<()> {
	syscall_int!(Munmap, addr, length)?;

	Ok(())
}

pub unsafe fn mprotect(addr: Ptr<()>, length: usize, prot: u32) -> Result<()> {
	syscall_int!(Mprotect, addr, length, prot)?;

	Ok(())
}

pub unsafe fn msync(addr: Ptr<()>, length: usize, flags: u32) -> Result<()> {
	syscall_int!(Msync, addr, length, flags)?;

	Ok(())
}

pub unsafe fn madvise(addr: Ptr<()>, length: usize, advice: u32) -> Result<()> {
	syscall_int!(Madvise, addr, length, advice)?;

	Ok(())
}

pub unsafe fn mlock(addr: Ptr<()>, length: usize) -> Result<()> {
	syscall_int!(Mlock, addr, length)?;

	Ok(())
}

pub unsafe fn munlock(addr: Ptr<()>, length: usize) -> Result<()> {
	syscall_int!(Munlock, addr, length)?;

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
	let addr = syscall_pointer!(Mremap, addr, old_length, new_length, flags, new_address)?;

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

	pub fn protect(&self, prot: u32) -> Result<()> {
		unsafe { mprotect(self.addr.into(), self.length, prot) }
	}

	pub fn sync(&self, flags: u32) -> Result<()> {
		unsafe { msync(self.addr.into(), self.length, flags) }
	}

	pub fn advise(&self, advice: u32) -> Result<()> {
		unsafe { madvise(self.addr.into(), self.length, advice) }
	}

	pub fn lock(&self) -> Result<()> {
		unsafe { mlock(self.addr.into(), self.length) }
	}

	pub fn unlock(&self) -> Result<()> {
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
