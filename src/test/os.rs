#[cfg(test)]
mod test {
	use std::{
		mem::transmute,
		os::fd::{FromRawFd, OwnedFd}
	};

	use enumflags2::make_bitflags;

	use crate::os::{
		error::{result_from_int, result_from_ptr, ErrorCodes},
		mman::{map_memory, MemoryAdvice, MemoryFlag, MemoryProtection, MemoryType},
		poll::{poll, PollFd, PollFlag},
		resource::{get_rlimit, Resource},
		time::{time, ClockId},
		unistd::close
	};

	#[test]
	fn test_close_inval() {
		close(unsafe { transmute(-2) }).unwrap_err();
		close(unsafe { OwnedFd::from_raw_fd(-2) }).unwrap_err();
		close(unsafe { OwnedFd::from_raw_fd(-20) }).unwrap_err();
		close(unsafe { OwnedFd::from_raw_fd(555) }).unwrap_err();
	}

	#[test]
	fn test_time() {
		assert!(time(ClockId::Monotonic).unwrap() > 0);
	}

	#[test]
	fn test_rlimit() {
		assert!(get_rlimit(Resource::Stack).unwrap().current > 0);
	}

	#[test]
	fn test_poll() {
		let mut fds = [PollFd {
			fd: 1,
			events: make_bitflags!(PollFlag::{Out}).bits() as u16,
			returned_events: 0
		}];

		poll(&mut fds, 0).unwrap();
		assert!(fds[0].returned_events().intersects(PollFlag::Out));
	}

	#[test]
	fn test_mmap() {
		let mut mem = map_memory(
			0x12345678000,
			16384,
			MemoryProtection::Read as u32,
			MemoryType::Private as u32 | MemoryFlag::Anonymous as u32,
			None,
			0
		)
		.unwrap();

		assert_eq!(mem.length(), 16384);
		assert_eq!(mem.addr(), 0x12345678000);

		mem.protect(MemoryProtection::Write as u32).unwrap();
		mem.advise(MemoryAdvice::Random as u32).unwrap();
		mem.lock().unwrap();
		mem.unlock().unwrap();

		drop(mem);
	}

	#[test]
	fn test_error() {
		result_from_int(-2).unwrap_err();
		result_from_int(0).unwrap();
		result_from_int(2).unwrap();
		result_from_ptr(-2).unwrap_err();
		result_from_ptr(0).unwrap();
		result_from_ptr(2).unwrap();
		result_from_ptr(-4095).unwrap_err();
		result_from_ptr(-4096).unwrap();
		result_from_ptr(isize::MIN).unwrap();
		result_from_ptr(isize::MAX).unwrap();
		assert_eq!(ErrorCodes::from_raw_os_error(2), ErrorCodes::NoEnt);
	}
}
