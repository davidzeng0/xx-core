pub mod buf_reader;
use std::str::from_utf8;

pub use buf_reader::*;
pub mod buf_writer;
pub use buf_writer::*;

pub mod read;
pub use read::*;
pub mod write;
pub use write::*;
pub mod seek;
pub use seek::*;
pub mod close;
pub use close::*;
pub mod split;
pub use split::*;

pub mod typed;
pub use typed::*;

use crate::{coroutines::*, error::*, opt::hint::unlikely, xx_core};

pub const DEFAULT_BUFFER_SIZE: usize = 16384;

pub fn check_utf8(buf: &[u8]) -> Result<()> {
	if from_utf8(buf).is_ok() {
		Ok(())
	} else {
		Err(Error::new(
			ErrorKind::InvalidData,
			"invalid UTF-8 found in stream"
		))
	}
}

#[async_fn]
pub async fn check_interrupt_if_zero<Context: AsyncContext>(len: usize) -> Result<usize> {
	if unlikely(len == 0) {
		check_interrupt().await?;
	}

	Ok(len)
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
