use std::os::fd::{AsRawFd, BorrowedFd, IntoRawFd, OwnedFd};

use crate::{
	macros::{import_sysdeps, syscall_impl},
	pointer::*
};

import_sysdeps!();

pub use platform::{SyscallNumber::*, *};

#[repr(transparent)]
pub struct SyscallParameter(pub usize);

macro_rules! impl_from_primitive {
	($type: ty) => {
		impl From<$type> for SyscallParameter {
			fn from(value: $type) -> Self {
				Self(value as usize)
			}
		}
	};
}

impl_from_primitive!(usize);
impl_from_primitive!(isize);
impl_from_primitive!(u64);
impl_from_primitive!(i64);
impl_from_primitive!(u32);
impl_from_primitive!(i32);
impl_from_primitive!(u16);
impl_from_primitive!(i16);
impl_from_primitive!(u8);
impl_from_primitive!(i8);

impl<T> From<Ptr<T>> for SyscallParameter {
	fn from(value: Ptr<T>) -> Self {
		Self::from(value.int_addr())
	}
}

impl<T> From<MutPtr<T>> for SyscallParameter {
	fn from(value: MutPtr<T>) -> Self {
		Self::from(value.int_addr())
	}
}

impl<T> From<&T> for SyscallParameter {
	fn from(value: &T) -> Self {
		Self::from(Ptr::from(value))
	}
}

impl<T> From<&mut T> for SyscallParameter {
	fn from(value: &mut T) -> Self {
		Self::from(MutPtr::from(value))
	}
}

impl<T> From<*const T> for SyscallParameter {
	fn from(value: *const T) -> Self {
		Self::from(Ptr::from(value))
	}
}

impl<T> From<*mut T> for SyscallParameter {
	fn from(value: *mut T) -> Self {
		Self::from(MutPtr::from(value))
	}
}

impl From<OwnedFd> for SyscallParameter {
	fn from(value: OwnedFd) -> Self {
		Self::from(value.into_raw_fd() as isize)
	}
}

impl From<BorrowedFd<'_>> for SyscallParameter {
	fn from(value: BorrowedFd<'_>) -> Self {
		Self::from(value.as_raw_fd() as isize)
	}
}

#[macro_export]
macro_rules! syscall_raw {
	($num: expr) => {
		$crate::os::syscall::syscall0($num as i32)
	};

	($num: expr, $arg1: expr) => {
		$crate::os::syscall::syscall1($num as i32, $arg1)
	};

	($num: expr, $arg1: expr, $arg2: expr) => {
		$crate::os::syscall::syscall2($num as i32, $arg1, $arg2)
	};

	($num: expr, $arg1: expr, $arg2: expr, $arg3: expr) => {
		$crate::os::syscall::syscall3($num as i32, $arg1, $arg2, $arg3)
	};

	($num: expr, $arg1: expr, $arg2: expr, $arg3: expr, $arg4: expr) => {
		$crate::os::syscall::syscall4($num as i32, $arg1, $arg2, $arg3, $arg4)
	};

	($num: expr, $arg1: expr, $arg2: expr, $arg3: expr, $arg4: expr, $arg5: expr) => {
		$crate::os::syscall::syscall5($num as i32, $arg1, $arg2, $arg3, $arg4, $arg5)
	};

	($num: expr, $arg1: expr, $arg2: expr, $arg3: expr, $arg4: expr, $arg5: expr, $arg6: expr) => {
		$crate::os::syscall::syscall6($num as i32, $arg1, $arg2, $arg3, $arg4, $arg5, $arg6)
	};
}

pub use syscall_raw;

#[macro_export]
macro_rules! syscall_int {
	($($arg: expr),+) => {
		$crate::os::error::result_from_int($crate::os::syscall::syscall_raw!($($arg),+))
	};
}

pub use syscall_int;

#[macro_export]
macro_rules! syscall_pointer {
	($($arg: expr),+) => {
		$crate::os::error::result_from_ptr($crate::os::syscall::syscall_raw!($($arg),+))
	};
}

pub use syscall_pointer;
