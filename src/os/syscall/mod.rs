use crate::sysdep::import_sysdeps;

import_sysdeps!();

pub use platform::*;

#[macro_export]
macro_rules! syscall_raw {
	($num: expr) => {
		$crate::os::syscall::syscall0($num as isize)
	};

	($num: expr, $arg1: expr) => {
		$crate::os::syscall::syscall1($num as isize, $arg1 as isize)
	};

	($num: expr, $arg1: expr, $arg2: expr) => {
		$crate::os::syscall::syscall2($num as isize, $arg1 as isize, $arg2 as isize)
	};

	($num: expr, $arg1: expr, $arg2: expr, $arg3: expr) => {
		$crate::os::syscall::syscall3(
			$num as isize,
			$arg1 as isize,
			$arg2 as isize,
			$arg3 as isize
		)
	};

	($num: expr, $arg1: expr, $arg2: expr, $arg3: expr, $arg4: expr) => {
		$crate::os::syscall::syscall4(
			$num as isize,
			$arg1 as isize,
			$arg2 as isize,
			$arg3 as isize,
			$arg4 as isize
		)
	};

	($num: expr, $arg1: expr, $arg2: expr, $arg3: expr, $arg4: expr, $arg5: expr) => {
		$crate::os::syscall::syscall5(
			$num as isize,
			$arg1 as isize,
			$arg2 as isize,
			$arg3 as isize,
			$arg4 as isize,
			$arg5 as isize
		)
	};

	($num: expr, $arg1: expr, $arg2: expr, $arg3: expr, $arg4: expr, $arg5: expr, $arg6: expr) => {
		$crate::os::syscall::syscall6(
			$num as isize,
			$arg1 as isize,
			$arg2 as isize,
			$arg3 as isize,
			$arg4 as isize,
			$arg5 as isize,
			$arg6 as isize
		)
	};
}

pub(crate) use syscall_raw;

#[macro_export]
macro_rules! syscall_int {
	($($arg: expr),+) => {
		$crate::os::error::result_from_int($crate::os::syscall::syscall_raw!($($arg),+))
	};
}

pub(crate) use syscall_int;

#[macro_export]
macro_rules! syscall_pointer {
	($($arg: expr),+) => {
		$crate::os::error::result_from_ptr($crate::os::syscall::syscall_raw!($($arg),+))
	};
}

pub(crate) use syscall_pointer;
