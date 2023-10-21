use std::{
	fmt::{self, Arguments},
	io::IoSlice
};

use super::bytes::BytesEncoding;
use crate::{
	async_std::ext::ext_func,
	coroutines::{
		async_fn, async_trait_fn, async_trait_impl,
		env::AsyncContext,
		runtime::{check_interrupt, get_context}
	},
	error::{Error, ErrorKind, Result},
	task::env::Handle,
	xx_core
};

#[async_trait_fn]
pub trait Write<Context: AsyncContext> {
	/// Write from `buf`, returning the amount of bytes read
	///
	/// Returning zero strictly means EOF, unless the buffer's size was zero
	async fn async_write(&mut self, buf: &[u8]) -> Result<usize>;

	/// Flush buffered data
	async fn async_flush(&mut self) -> Result<()>;

	/// Try to write the entire buffer, returning on I/O error, interrupt, or
	/// EOF
	///
	/// On interrupted, returns the number of bytes read if it is not zero
	async fn async_write_all(&mut self, buf: &[u8]) -> Result<usize> {
		let mut wrote = 0;

		while wrote < buf.len() {
			match self.write(&buf[wrote..]).await {
				Ok(0) => break,
				Ok(n) => wrote += n,
				Err(err) => {
					if err.is_interrupted() {
						break;
					}

					return Err(err);
				}
			}
		}

		if wrote == 0 {
			check_interrupt().await?;
		}

		Ok(wrote)
	}

	fn is_write_vectored(&self) -> bool {
		false
	}

	async fn async_write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
		let buf = match bufs.iter().find(|b| !b.is_empty()) {
			Some(buf) => buf,
			None => return Ok(0)
		};

		self.write(&**buf).await
	}
}

struct FmtAdapter<'a, Context: AsyncContext, T: ?Sized + 'a> {
	inner: &'a mut T,
	context: Handle<Context>,
	wrote: usize,
	error: Option<Error>
}

#[async_fn]
impl<'a, T: ?Sized + Write<Context>, Context: AsyncContext> FmtAdapter<'a, Context, T> {
	pub async fn new(inner: &'a mut T) -> FmtAdapter<'a, Context, T> {
		Self {
			inner,
			context: get_context().await,
			wrote: 0,
			error: None
		}
	}

	pub async fn write_args(&mut self, args: Arguments<'_>) -> Result<usize> {
		match fmt::write(self, args) {
			Ok(()) => Ok(self.wrote),
			Err(_) => Err(self
				.error
				.take()
				.unwrap_or(Error::new(ErrorKind::Other, "Formatter error")))
		}
	}
}

impl<T: ?Sized + Write<Context>, Context: AsyncContext> fmt::Write for FmtAdapter<'_, Context, T> {
	fn write_str(self: &mut Self, s: &str) -> fmt::Result {
		match self.context.run(self.inner.write_string(s)) {
			Err(err) => {
				self.error = Some(err);

				Err(fmt::Error)
			}

			Ok(n) => {
				self.wrote += n;

				Ok(())
			}
		}
	}
}

pub trait WriteExt<Context: AsyncContext>: Write<Context> {
	ext_func!(write(self: &mut Self, buf: &[u8]) -> Result<usize>);

	ext_func!(flush(self: &mut Self) -> Result<()>);

	ext_func!(write_all(self: &mut Self, buf: &[u8]) -> Result<usize>);

	ext_func!(write_vectored(self: &mut Self, bufs: &[IoSlice<'_>]) -> Result<usize>);

	#[async_trait_impl]
	async fn write_fmt(&mut self, args: Arguments<'_>) -> Result<usize> {
		FmtAdapter::new(self).await.write_args(args).await
	}

	#[async_trait_impl]
	async fn write_string(&mut self, buf: &str) -> Result<usize> {
		self.write_all(buf.as_bytes()).await
	}

	#[async_trait_impl]
	async fn write_char(&mut self, ch: char) -> Result<usize> {
		let mut buf = [0u8; 4];

		self.write_string(ch.encode_utf8(&mut buf)).await
	}

	/// Write the number `val`, as little endian bytes
	#[inline(always)]
	#[async_trait_impl]
	async fn write_le<T: BytesEncoding<N>, const N: usize>(&mut self, val: T) -> Result<usize> {
		self.write_all(&val.to_bytes_le()).await
	}

	/// Write the number `val`, as big endian bytes
	#[inline(always)]
	#[async_trait_impl]
	async fn write_be<T: BytesEncoding<N>, const N: usize>(&mut self, val: T) -> Result<usize> {
		self.write_all(&val.to_bytes_be()).await
	}
}

impl<Context: AsyncContext, T: ?Sized + Write<Context>> WriteExt<Context> for T {}
