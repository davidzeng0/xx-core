use std::str::from_utf8;

mod buf_reader;
pub use buf_reader::*;
mod buf_writer;
pub use buf_writer::*;

mod read;
pub use read::*;
mod write;
pub use write::*;
mod seek;
pub use seek::*;
mod split;
pub use split::*;

mod typed;
use std::{
	io::{IoSlice, IoSliceMut, SeekFrom},
	marker::PhantomData,
	ptr::copy
};

pub use typed::*;

use super::*;
use crate::{coroutines::*, error::*, opt::hint::*};

pub const DEFAULT_BUFFER_SIZE: usize = 16384;

pub fn invalid_utf8_error() -> Error {
	Error::new(ErrorKind::InvalidData, "invalid UTF-8 found in stream")
}

pub fn check_utf8(buf: &[u8]) -> Result<()> {
	if from_utf8(buf).is_ok() {
		Ok(())
	} else {
		Err(invalid_utf8_error())
	}
}

#[async_fn]
pub async fn check_interrupt_if_zero(len: usize) -> Result<usize> {
	if unlikely(len == 0) {
		check_interrupt().await?;
	}

	Ok(len)
}

pub fn unexpected_end_of_stream() -> Error {
	Error::new(ErrorKind::UnexpectedEof, "Unexpected end of stream")
}

#[async_fn]
pub async fn short_io_error_unless_interrupt() -> Error {
	check_interrupt()
		.await
		.err()
		.unwrap_or_else(|| unexpected_end_of_stream())
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
			let buf = unsafe { $buf.get_unchecked_mut(0..min) };

			read_into!(buf);

			buf
		};
	};
}

#[macro_export]
macro_rules! write_from {
	($buf: ident) => {
		if $crate::opt::hint::unlikely($buf.len() == 0) {
			return Ok(0);
		}
	};
}
