use xx_core_macros::wrapper_functions;

use self::time::TimeSpec;
use super::*;

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum PollFlag {
		/// There is data to read.
		In        = 1 << 0,

		/// There is urgent data to read.
		Priority  = 1 << 1,

		/// Writing now will not block.
		Out       = 1 << 2,

		/// Error condition.
		Error     = 1 << 3,

		/// Hung up.
		HangUp    = 1 << 4,

		/// Invalid polling request.
		Invalid   = 1 << 5,

		/// Normal data may be read.
		ReadNorm  = 1 << 6,

		/// Priority data may be read.
		ReadBand  = 1 << 7,

		/// Writing now will not block.
		WriteNorm = 1 << 8,

		/// Priority data may be written.
		WriteBand = 1 << 9,

		/// Extensions for Linux
		Message   = 1 << 10,
		Remove    = 1 << 12,
		RdHangUp  = 1 << 13
	}
}

define_struct! {
	pub struct PollFd {
		pub fd: i32,
		pub events: u16,
		pub returned_events: u16
	}
}

impl PollFd {
	pub fn events(&self) -> BitFlags<PollFlag> {
		unsafe { BitFlags::from_bits_unchecked(self.events as u32) }
	}

	pub fn returned_events(&self) -> BitFlags<PollFlag> {
		unsafe { BitFlags::from_bits_unchecked(self.returned_events as u32) }
	}
}

#[repr(transparent)]
pub struct BorrowedPollFd<'a> {
	poll_fd: PollFd,
	phantom: PhantomData<&'a ()>
}

impl<'a> BorrowedPollFd<'a> {
	wrapper_functions! {
		inner = self.poll_fd;

		pub fn events(&self) -> BitFlags<PollFlag>;
		pub fn returned_events(&self) -> BitFlags<PollFlag>;
	}

	pub fn new(fd: BorrowedFd<'a>, events: BitFlags<PollFlag>) -> Self {
		Self {
			poll_fd: PollFd {
				fd: fd.as_raw_fd(),
				events: events.bits() as u16,
				returned_events: 0
			},
			phantom: PhantomData
		}
	}
}

pub unsafe fn poll_raw(fds: &mut [PollFd], timeout: i32) -> Result<u32> {
	assert!(timeout >= 0);

	let ts = TimeSpec::from_ms(timeout as u64);
	let events =
		unsafe { syscall_int!(Ppoll, fds.as_mut_ptr(), fds.len(), &ts, Ptr::<()>::null())? };

	Ok(events as u32)
}

pub fn poll(fds: &mut [BorrowedPollFd<'_>], timeout: i32) -> Result<u32> {
	unsafe { poll_raw(transmute(fds), timeout) }
}
