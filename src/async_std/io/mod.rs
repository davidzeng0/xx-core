use std::{
	io::{IoSlice, IoSliceMut, SeekFrom},
	mem::{take, transmute},
	str::from_utf8
};

mod read;
pub use read::*;
mod write;
pub use write::*;
mod seek;
pub use seek::*;
mod split;
pub use split::*;

mod buf_reader;
pub use buf_reader::*;
mod buf_writer;
pub use buf_writer::*;

pub mod typed;

use super::*;
use crate::{coroutines::*, error::*, opt::hint::*};

pub const DEFAULT_BUFFER_SIZE: usize = 16384;

pub fn check_utf8(buf: &[u8]) -> Result<()> {
	if from_utf8(buf).is_ok() {
		Ok(())
	} else {
		Err(Core::InvalidUtf8.new())
	}
}

#[asynchronous]
pub async fn check_interrupt_if_zero(len: usize) -> Result<usize> {
	if unlikely(len == 0) {
		check_interrupt().await?;
	}

	Ok(len)
}

pub fn advance_slices(bufs: &mut &mut [IoSlice<'_>], mut amount: usize) {
	let mut remove = 0;

	for buf in bufs.iter_mut() {
		if amount >= buf.len() {
			amount -= buf.len();
			remove += 1;
		} else {
			let left = &buf[amount..];

			/* Safety: this mimics the IoSliceMut::advance function, no lifetimes are
			 * violated here */
			*buf = IoSlice::new(unsafe { transmute(left) });

			break;
		}
	}

	*bufs = &mut take(bufs)[remove..];

	debug_assert_eq!(amount, 0);
}

pub fn advance_slices_mut(bufs: &mut &mut [IoSliceMut<'_>], mut amount: usize) {
	let mut remove = 0;

	for buf in bufs.iter_mut() {
		if amount >= buf.len() {
			amount -= buf.len();
			remove += 1;
		} else {
			let left = &mut buf[amount..];

			/* Safety: this mimics the IoSliceMut::advance function, no lifetimes are
			 * violated here */
			*buf = IoSliceMut::new(unsafe { transmute(left) });

			break;
		}
	}

	*bufs = &mut take(bufs)[remove..];

	debug_assert_eq!(amount, 0);
}

pub fn length_check(buf: &[u8], len: usize) -> usize {
	debug_assert!(len <= buf.len());

	len
}

#[asynchronous]
pub async fn short_io_error_unless_interrupt() -> Error {
	check_interrupt()
		.await
		.err()
		.unwrap_or_else(|| Core::UnexpectedEof.new())
}

#[macro_export]
macro_rules! read_into {
	($buf: ident) => {
		if $crate::opt::hint::unlikely($buf.len() == 0) {
			return Ok(0);
		}
	};

	($buf: ident, $limit: expr) => {
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
	($buf: ident) => {
		if $crate::opt::hint::unlikely($buf.len() == 0) {
			return Ok(0);
		}
	};
}

pub use write_from;
