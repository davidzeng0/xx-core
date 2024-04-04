pub mod epoll;
pub mod error;
pub mod fcntl;
pub mod inet;
pub mod io_uring;
pub mod iovec;
pub mod mman;
pub mod openat;
pub mod openat2;
pub mod poll;
pub mod resource;
pub mod signal;
pub mod socket;
pub mod stat;
pub mod syscall;
pub mod tcp;
pub mod time;
pub mod unistd;

use std::{
	marker::PhantomData,
	mem::{size_of, size_of_val, transmute},
	os::fd::*
};

use enumflags2::*;
use syscall::*;

use crate::{error::*, macros::syscall_define, pointer::*};

pub const INVALID_FD: RawFd = -1;

pub mod raw {
	use super::*;

	define_struct! {
		pub struct RawBuf {
			pub ptr: MutPtr<()>,
			pub len: usize
		}
	}

	#[repr(transparent)]
	#[derive(Default, Debug)]
	pub struct BorrowedRawBuf<'a, const MUT: bool> {
		pub buf: RawBuf,
		pub phantom: PhantomData<&'a ()>
	}
}

pub type RawBuf<'a> = raw::BorrowedRawBuf<'a, false>;
pub type MutRawBuf<'a> = raw::BorrowedRawBuf<'a, true>;

impl IntoRawArray for RawBuf<'_> {
	type Length = usize;
	type Pointer = Ptr<()>;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length) {
		(self.buf.ptr.cast_const(), self.buf.len)
	}
}

impl IntoRawArray for MutRawBuf<'_> {
	type Length = usize;
	type Pointer = MutPtr<()>;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length) {
		(self.buf.ptr, self.buf.len)
	}
}

impl RawBuf<'_> {
	#[must_use]
	pub const fn from_parts(value: Ptr<()>, len: usize) -> Self {
		Self {
			buf: raw::RawBuf { ptr: value.cast_mut(), len },
			phantom: PhantomData
		}
	}
}

impl<'a> RawBuf<'a> {
	#[must_use]
	pub const fn cast_mut(self) -> MutRawBuf<'a> {
		MutRawBuf { buf: self.buf, phantom: self.phantom }
	}
}

impl MutRawBuf<'_> {
	#[must_use]
	pub const fn from_parts(value: MutPtr<()>, len: usize) -> Self {
		Self {
			buf: raw::RawBuf { ptr: value, len },
			phantom: PhantomData
		}
	}
}

impl<'a, T> From<&'a [T]> for RawBuf<'a> {
	fn from(value: &'a [T]) -> Self {
		Self {
			buf: raw::RawBuf {
				ptr: ptr!(value.as_ptr()).cast_mut().cast(),
				len: size_of_val(value)
			},
			phantom: PhantomData
		}
	}
}

impl<'a, T> From<&'a T> for RawBuf<'a> {
	fn from(value: &'a T) -> Self {
		Self {
			buf: raw::RawBuf {
				ptr: ptr!(value).cast_mut().cast(),
				len: size_of::<T>()
			},
			phantom: PhantomData
		}
	}
}

impl<'a, T> From<&'a mut [T]> for MutRawBuf<'a> {
	fn from(value: &'a mut [T]) -> Self {
		Self {
			buf: raw::RawBuf {
				ptr: ptr!(value.as_mut_ptr()).cast(),
				len: size_of_val(value)
			},
			phantom: PhantomData
		}
	}
}

impl<'a, T> From<&'a mut T> for MutRawBuf<'a> {
	fn from(value: &'a mut T) -> Self {
		Self {
			buf: raw::RawBuf { ptr: ptr!(value).cast(), len: size_of::<T>() },
			phantom: PhantomData
		}
	}
}

macro_rules! define_into_raw_repr {
	($name: ident #[repr($repr:ty)] $(#$rest:tt)*) => {
		impl $crate::os::syscall::IntoRaw for $name {
			type Raw = $repr;

			fn into_raw(self) -> $repr {
				self as $repr
			}
		}

		define_into_raw_repr!($name $(#$rest)*);
	};

	($name: ident #$attr:tt $(#$rest:tt)*) => {
		define_into_raw_repr!($name $(#$rest)*);
	};

	($name: ident) => {};
}

use define_into_raw_repr;

macro_rules! define_enum {
	(
		$(#$attrs: tt)*
		$vis: vis
		enum $name: ident
		$($rest: tt)*
	) => {
		#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug, ::num_derive::FromPrimitive)]
		$(#$attrs)*
		$vis enum $name $($rest)*

		define_into_raw_repr!($name $(#$attrs)*);
	}
}

use define_enum;

macro_rules! define_struct {
	(
		$(#$attrs: tt)*
		$vis: vis
		struct $name: ident
		$($rest: tt)*
	) => {
		#[derive(Clone, Copy, PartialEq, Eq, Debug)]
		#[repr(C)]
		$(#$attrs)*
		$vis struct $name $($rest)*

		#[allow(deprecated)]
		impl ::std::default::Default for $name {
			fn default() -> Self {
				/* Safety: repr(C) */
				unsafe { ::std::mem::zeroed() }
			}
		}
	}
}

use define_struct;

macro_rules! define_union {
	(
		$(#$attrs: tt)*
		$vis: vis
		union $name: ident
		$($rest: tt)*
	) => {
		#[derive(Clone, Copy, Eq)]
		#[repr(C)]
		$(#$attrs)*
		$vis union $name $($rest)*

		#[allow(deprecated)]
		impl ::std::default::Default for $name {
			fn default() -> Self {
				/* Safety: repr(C) */
				unsafe { ::std::mem::zeroed() }
			}
		}

		#[allow(deprecated)]
		impl ::std::cmp::PartialEq for $name {
			fn eq(&self, other: &Self) -> bool {
				::std::ptr::eq(self, other)
			}
		}

		#[allow(deprecated)]
		impl ::std::fmt::Debug for $name {
			fn fmt(&self, fmt: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
				fmt.debug_struct(::std::stringify!($name)).finish()
			}
		}
	}
}

use define_union;
