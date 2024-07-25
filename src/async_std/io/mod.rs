#![warn(unsafe_code)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{IoSlice, IoSliceMut, SeekFrom};
use std::mem::{take, transmute};
use std::str::from_utf8;

use super::*;
use crate::io::*;
use crate::macros::{macro_each, sealed_trait};
use crate::opt::hint::*;

macro_each!(sealed_trait, (for Read), (for BufRead), (for Write));

pub mod buf_reader;
pub mod buf_writer;
pub mod read;
pub mod seek;
pub mod split;
pub mod typed;
pub mod write;

#[doc(inline)]
pub use {buf_reader::*, buf_writer::*, read::*, seek::*, split::*, write::*};

/// The default buffer size (16 KiB) for buffered I/O
pub const DEFAULT_BUFFER_SIZE: usize = 0x4000;

/// If `len` is zero, checks if the current async task is interrupted
#[asynchronous]
pub async fn check_interrupt_if_zero(len: usize) -> Result<usize> {
	if len == 0 {
		check_interrupt().await?;
	}

	Ok(len)
}

/// Advances a slice of slices
///
/// See also [`std::io::IoSlice::advance_slices`]
///
/// # Panics
/// If `amount` is greater than the length of the buffers combined
pub fn advance_slices(bufs: &mut &mut [IoSlice<'_>], mut amount: usize) {
	let mut remove = 0;

	for buf in bufs.iter_mut() {
		if let Some(amt) = amount.checked_sub(buf.len()) {
			amount = amt;

			#[allow(clippy::arithmetic_side_effects)]
			(remove += 1);
		} else {
			let left = &buf[amount..];

			#[allow(unsafe_code)]
			/* Safety: this mimics the IoSliceMut::advance function, no lifetimes are
			 * violated here */
			(*buf = IoSlice::new(unsafe { transmute(left) }));
			amount = 0;

			break;
		}
	}

	*bufs = &mut take(bufs)[remove..];

	assert_eq!(amount, 0);
}

/// Advances a slice of slices
///
/// See also [`std::io::IoSlice::advance_slices`]
///
/// # Panics
/// If `amount` is greater than the length of the buffers combined
pub fn advance_slices_mut(bufs: &mut &mut [IoSliceMut<'_>], mut amount: usize) {
	let mut remove = 0;

	for buf in bufs.iter_mut() {
		if let Some(amt) = amount.checked_sub(buf.len()) {
			amount = amt;

			#[allow(clippy::arithmetic_side_effects)]
			(remove += 1);
		} else {
			let left = &mut buf[amount..];

			#[allow(unsafe_code)]
			/* Safety: this mimics the IoSliceMut::advance function, no lifetimes are
			 * violated here */
			(*buf = IoSliceMut::new(unsafe { transmute(left) }));
			amount = 0;

			break;
		}
	}

	*bufs = &mut take(bufs)[remove..];

	assert_eq!(amount, 0);
}

/// Checks that `len` <= `buf.len()`
///
/// Returns `len`
///
/// # Panics
/// if `len` > `buf.len()`
#[allow(clippy::must_use_candidate)]
pub fn length_check(buf: &[u8], len: usize) -> usize {
	assert!(len <= buf.len());

	len
}

/// Returns [`UnexpectedEof`] unless the current task is interrupted
///
/// [`UnexpectedEof`]: ErrorKind::UnexpectedEof
#[asynchronous]
pub async fn short_io_error_unless_interrupt() -> Error {
	check_interrupt()
		.await
		.err()
		.unwrap_or_else(|| ErrorKind::UnexpectedEof.into())
}

#[macro_export]
/// Utility macro for returning early if `buf` is empty
macro_rules! read_into {
	($buf:ident) => {
		if $crate::opt::hint::unlikely($buf.is_empty()) {
			return Ok(0);
		}
	};

	($buf:ident, $limit:expr) => {
		let $buf = {
			let min = $buf.len().min($limit);
			let buf = &mut $buf[0..min];

			read_into!(buf);

			buf
		};
	};
}

pub use read_into;

#[macro_export]
/// Utility macro for returning early if `buf` is empty
macro_rules! write_from {
	($buf:ident) => {
		if $crate::opt::hint::unlikely($buf.is_empty()) {
			return Ok(0);
		}
	};
}

pub use write_from;
