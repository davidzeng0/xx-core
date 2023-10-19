use super::bytes::BytesEncoding;
use crate::{
	async_std::ext::ext_func,
	coroutines::{async_trait_fn, async_trait_impl, env::AsyncContext, runtime::check_interrupt},
	error::Result,
	xx_core
};

#[async_trait_fn]
pub trait Write<Context: AsyncContext> {
	/// Write from `buf`, returning the amount of bytes read
	///
	/// Returning zero strictly means EOF
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
}

pub trait WriteExt<Context: AsyncContext>: Write<Context> {
	ext_func!(write(self: &mut Self, buf: &[u8]) -> Result<usize>);

	ext_func!(flush(self: &mut Self) -> Result<()>);

	ext_func!(write_all(self: &mut Self, buf: &[u8]) -> Result<usize>);

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
