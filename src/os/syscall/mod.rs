use enumflags2::*;

use crate::{
	error::*,
	macros::{import_sysdeps, syscall_impl},
	pointer::*
};

import_sysdeps!();

pub use platform::*;
pub use SyscallNumber::*;

use super::{error::*, *};

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Copy)]
pub struct SyscallParameter(pub usize);

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Copy)]
pub struct SyscallResult(pub isize);

macro_rules! impl_from_primitive {
	($type:ty) => {
		impl From<$type> for SyscallParameter {
			fn from(value: $type) -> Self {
				SyscallParameter(value as usize)
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

impl<T, const MUTABLE: bool> From<Pointer<T, MUTABLE>> for SyscallParameter {
	fn from(value: Pointer<T, MUTABLE>) -> Self {
		value.int_addr().into()
	}
}

macro_rules! impl_pointer {
	($type:ty) => {
		impl<T> From<$type> for SyscallParameter {
			fn from(value: $type) -> Self {
				Pointer::from(value).into()
			}
		}
	};
}

impl_pointer!(&T);
impl_pointer!(&mut T);
impl_pointer!(*const T);
impl_pointer!(*mut T);

impl From<SyscallResult> for Result<()> {
	fn from(val: SyscallResult) -> Self {
		result_from_int(val.0).map(|_| ())
	}
}

macro_rules! impl_primitive_from {
	($type:ty) => {
		impl From<SyscallResult> for Result<$type> {
			fn from(val: SyscallResult) -> Self {
				#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
				result_from_int(val.0).map(|result| result as $type)
			}
		}
	};
}

impl_primitive_from!(u32);
impl_primitive_from!(i32);
impl_primitive_from!(usize);
impl_primitive_from!(isize);

impl From<SyscallResult> for Result<OwnedFd> {
	fn from(val: SyscallResult) -> Self {
		result_from_int(val.0).map(|raw_fd| {
			/* Safety: guaranteed by syscall declaration */
			#[allow(clippy::cast_possible_truncation)]
			unsafe {
				OwnedFd::from_raw_fd(raw_fd as i32)
			}
		})
	}
}

impl<T, const MUTABLE: bool> From<SyscallResult> for Result<Pointer<T, MUTABLE>> {
	fn from(val: SyscallResult) -> Self {
		result_from_ptr(val.0).map(|addr| Pointer::from_int_addr(addr))
	}
}

pub trait IntoRaw {
	type Raw: Into<SyscallParameter>;

	fn into_raw(self) -> Self::Raw;
}

impl<T: Into<SyscallParameter>> IntoRaw for T {
	type Raw = Self;

	fn into_raw(self) -> Self {
		self
	}
}

impl<T: BitFlag<Numeric = Numeric>, Numeric: Into<SyscallParameter>> IntoRaw for BitFlags<T> {
	type Raw = T::Numeric;

	fn into_raw(self) -> Self::Raw {
		self.bits()
	}
}

impl IntoRaw for OwnedFd {
	type Raw = RawFd;

	fn into_raw(self) -> i32 {
		self.into_raw_fd()
	}
}

impl IntoRaw for BorrowedFd<'_> {
	type Raw = RawFd;

	fn into_raw(self) -> i32 {
		self.as_raw_fd()
	}
}

impl IntoRaw for Option<BorrowedFd<'_>> {
	type Raw = RawFd;

	fn into_raw(self) -> i32 {
		self.as_ref().map_or(INVALID_FD, AsRawFd::as_raw_fd)
	}
}

impl<T> IntoRaw for Option<&T> {
	type Raw = Ptr<T>;

	fn into_raw(self) -> Ptr<T> {
		self.map_or(Ptr::null(), Ptr::from)
	}
}

impl<T> IntoRaw for Option<&mut T> {
	type Raw = MutPtr<T>;

	fn into_raw(self) -> MutPtr<T> {
		self.map_or(MutPtr::null(), MutPtr::from)
	}
}

pub trait IntoRawArray {
	type Pointer: IntoRaw;
	type Length: IntoRaw;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length);
}

impl<T> IntoRawArray for &[T] {
	type Length = usize;
	type Pointer = Ptr<T>;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length) {
		(self.as_ptr().into(), self.len())
	}
}

impl<T> IntoRawArray for &mut [T] {
	type Length = usize;
	type Pointer = MutPtr<T>;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length) {
		(self.as_mut_ptr().into(), self.len())
	}
}

impl<T> IntoRawArray for Option<&[T]> {
	type Length = usize;
	type Pointer = Ptr<T>;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length) {
		self.map_or((Ptr::null(), 0), IntoRawArray::into_raw_array)
	}
}

impl<T> IntoRawArray for Option<&mut [T]> {
	type Length = usize;
	type Pointer = MutPtr<T>;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length) {
		self.map_or((MutPtr::null(), 0), IntoRawArray::into_raw_array)
	}
}

#[macro_export]
#[allow(clippy::module_name_repetitions)]
macro_rules! syscall_raw {
	($num:expr) => {
		$crate::os::syscall::syscall0($num)
	};

	($num:expr, $arg1:expr) => {
		$crate::os::syscall::syscall1($num, $arg1)
	};

	($num:expr, $arg1:expr, $arg2:expr) => {
		$crate::os::syscall::syscall2($num, $arg1, $arg2)
	};

	($num:expr, $arg1:expr, $arg2:expr, $arg3:expr) => {
		$crate::os::syscall::syscall3($num, $arg1, $arg2, $arg3)
	};

	($num:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr) => {
		$crate::os::syscall::syscall4($num, $arg1, $arg2, $arg3, $arg4)
	};

	($num:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr) => {
		$crate::os::syscall::syscall5($num, $arg1, $arg2, $arg3, $arg4, $arg5)
	};

	($num:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr) => {
		$crate::os::syscall::syscall6($num, $arg1, $arg2, $arg3, $arg4, $arg5, $arg6)
	};
}

#[allow(clippy::module_name_repetitions)]
pub use syscall_raw;

#[macro_export]
#[allow(clippy::module_name_repetitions)]
macro_rules! syscall_int {
	($($arg: expr),+) => {
		$crate::os::error::result_from_int($crate::os::syscall::syscall_raw!($($arg),+))
	};
}

#[allow(clippy::module_name_repetitions)]
pub use syscall_int;

#[macro_export]
#[allow(clippy::module_name_repetitions)]
macro_rules! syscall_pointer {
	($($arg: expr),+) => {
		$crate::os::error::result_from_ptr($crate::os::syscall::syscall_raw!($($arg),+))
	};
}

#[allow(clippy::module_name_repetitions)]
pub use syscall_pointer;
