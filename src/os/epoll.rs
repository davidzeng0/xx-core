use super::{poll::PollFlag, *};

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum EventPollFlag {
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
}

define_enum! {
	#[repr(u32)]
	pub enum CtlOp {
		/// Add a file descriptor to the interface.
		Add = 1,

		/// Remove a file descriptor from the interface.
		Del,

		/// Change file descriptor epoll_event structure.
		Mod
	}
}

define_struct! {
	pub struct EpollEvent {
		pub events: u32,
		pub data: u64
	}
}

pub struct EventPoll {
	fd: OwnedFd
}

impl EventPoll {
	pub fn create(flags: u32) -> Result<Self> {
		unsafe {
			let fd = syscall_int!(EpollCreate1, flags)?;

			Ok(Self { fd: OwnedFd::from_raw_fd(fd as i32) })
		}
	}

	pub fn ctl(&self, op: CtlOp, fd: BorrowedFd<'_>, event: &mut EpollEvent) -> Result<()> {
		unsafe { syscall_int!(EpollCtl, self.fd.as_fd(), op as u32, fd, event)? };

		Ok(())
	}

	pub fn wait(&self, events: &mut [EpollEvent], timeout: i32) -> Result<u32> {
		let events = unsafe {
			syscall_int!(
				EpollPwait,
				self.fd.as_fd(),
				events.as_mut_ptr(),
				events.len(),
				timeout,
				Ptr::<()>::null()
			)?
		};

		Ok(events as u32)
	}
}
