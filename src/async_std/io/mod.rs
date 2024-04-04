use std::{
	io::{IoSlice, IoSliceMut, SeekFrom},
	mem::{take, transmute},
	str::from_utf8
};

use super::*;
use crate::{
	coroutines::*,
	error::*,
	macros::{macro_each, seal_trait},
	opt::hint::*
};

macro_each!(seal_trait, Read, BufRead, Write);

pub mod buf_reader;
pub mod buf_writer;
pub mod read;
pub mod seek;
pub mod split;
pub mod typed;
pub mod write;

pub use buf_reader::*;
pub use buf_writer::*;
pub use read::*;
pub use seek::*;
pub use split::*;
pub use write::*;

pub const DEFAULT_BUFFER_SIZE: usize = 0x4000;

pub fn check_utf8(buf: &[u8]) -> Result<()> {
	match from_utf8(buf) {
		Ok(_) => Ok(()),
		Err(_) => Err(Core::InvalidUtf8.into())
	}
}

#[asynchronous]
pub async fn check_interrupt_if_zero(len: usize) -> Result<usize> {
	if unlikely(len == 0) {
		check_interrupt().await?;
	}

	Ok(len)
}

/// # Panics
/// if `amount` is greater than the length of the buffers combined
pub fn advance_slices(bufs: &mut &mut [IoSlice<'_>], mut amount: usize) {
	let mut remove = 0;

	for buf in bufs.iter_mut() {
		if let Some(amt) = amount.checked_sub(buf.len()) {
			amount = amt;

			#[allow(clippy::arithmetic_side_effects)]
			(remove += 1);
		} else {
			let left = &buf[amount..];

			/* Safety: this mimics the IoSliceMut::advance function, no lifetimes are
			 * violated here */
			*buf = IoSlice::new(unsafe { transmute(left) });
			amount = 0;

			break;
		}
	}

	*bufs = &mut take(bufs)[remove..];

	assert_eq!(amount, 0);
}

/// # Panics
/// if `amount` is greater than the length of the buffers combined
pub fn advance_slices_mut(bufs: &mut &mut [IoSliceMut<'_>], mut amount: usize) {
	let mut remove = 0;

	for buf in bufs.iter_mut() {
		if let Some(amt) = amount.checked_sub(buf.len()) {
			amount = amt;

			#[allow(clippy::arithmetic_side_effects)]
			(remove += 1);
		} else {
			let left = &mut buf[amount..];

			/* Safety: this mimics the IoSliceMut::advance function, no lifetimes are
			 * violated here */
			*buf = IoSliceMut::new(unsafe { transmute(left) });
			amount = 0;

			break;
		}
	}

	*bufs = &mut take(bufs)[remove..];

	assert_eq!(amount, 0);
}

/// # Panics
/// if `len` > `buf.len()`
#[allow(clippy::must_use_candidate)]
pub fn length_check(buf: &[u8], len: usize) -> usize {
	assert!(len <= buf.len());

	len
}

#[asynchronous]
pub async fn short_io_error_unless_interrupt() -> Error {
	check_interrupt()
		.await
		.err()
		.unwrap_or_else(|| Core::UnexpectedEof.into())
}

#[macro_export]
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
macro_rules! write_from {
	($buf:ident) => {
		if $crate::opt::hint::unlikely($buf.is_empty()) {
			return Ok(0);
		}
	};
}

pub use write_from;
