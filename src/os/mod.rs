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
pub mod socket;
pub mod stat;
pub mod syscall;
pub mod tcp;
pub mod time;
pub mod unistd;

use std::{
	marker::PhantomData,
	mem::{size_of, transmute},
	os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd}
};

use enumflags2::{bitflags, BitFlags};
use syscall::*;

use crate::{error::*, pointer::*};

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
	}
}

pub(self) use define_enum;

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
				unsafe { ::std::mem::zeroed() }
			}
		}
	}
}

pub(self) use define_struct;

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
				fmt.debug_struct(stringify!($name)).finish()
			}
		}
	}
}

pub(self) use define_union;
