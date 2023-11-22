#[cfg(test)]
mod test {
	use std::{
		mem::transmute,
		os::fd::{BorrowedFd, FromRawFd, OwnedFd}
	};

	use enumflags2::make_bitflags;

	use crate::{
		os::{
			error::{result_from_int, result_from_ptr, ErrorCodes},
			mman::{MemoryAdvice, MemoryFlag, MemoryMap, MemoryProtection, MemoryType},
			poll::{poll, PollFd, PollFlag},
			resource::{get_rlimit, Resource},
			time::{time, ClockId},
			unistd::close
		},
		pointer::{MutPtr, Ptr}
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
		let mut fds = [PollFd::new(
			unsafe { BorrowedFd::borrow_raw(1) },
			make_bitflags!(PollFlag::{Out})
		)];

		poll(&mut fds, 0).unwrap();
		assert!(fds[0].returned_events().intersects(PollFlag::Out));
	}

	#[test]
	fn test_mmap() {
		let mut mem = MemoryMap::map(
			Some(Ptr::from_int_addr(0x12345678000)),
			16384,
			MemoryProtection::Read as u32,
			MemoryType::Private as u32 | MemoryFlag::Anonymous as u32,
			None,
			0
		)
		.unwrap();

		assert_eq!(mem.length(), 16384);
		assert_eq!(mem.addr(), MutPtr::from_int_addr(0x12345678000));

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
