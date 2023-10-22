use std::{io::SeekFrom, marker::PhantomData};

use super::*;
use crate::{coroutines::*, error::*, opt::hint::likely, xx_core};

pub struct BufWriter<Context: AsyncContext, W: Write<Context>> {
	inner: W,

	buf: Vec<u8>,
	pos: usize,

	phantom: PhantomData<Context>
}

#[async_fn]
impl<Context: AsyncContext, W: Write<Context>> BufWriter<Context, W> {
	/// Discard all buffered data
	#[inline]
	fn discard(&mut self) {
		self.pos = 0;

		unsafe {
			self.buf.set_len(0);
		}
	}

	/// Reads from `buf` into our internal buffer
	#[inline]
	fn write_buffered(&mut self, buf: &[u8]) -> usize {
		self.buf.extend_from_slice(buf);

		buf.len()
	}

	/// Flushes the buffer without flushing downstream
	async fn flush_buf(&mut self) -> Result<()> {
		while self.pos < self.buf.len() {
			check_interrupt().await?;

			let wrote = self.inner.write(&self.buf[self.pos..]).await?;

			if likely(wrote != 0) {
				self.pos += wrote;
			} else {
				return Err(Error::new(
					ErrorKind::WriteZero,
					"Write returned EOF while flushing"
				));
			}
		}

		self.discard();

		Ok(())
	}

	pub fn new(inner: W) -> Self {
		Self::with_capacity(inner, DEFAULT_BUFFER_SIZE)
	}

	pub fn with_capacity(inner: W, capacity: usize) -> Self {
		Self::from_parts(inner, Vec::with_capacity(capacity))
	}

	pub fn from_parts(inner: W, buf: Vec<u8>) -> Self {
		BufWriter { inner, buf, pos: 0, phantom: PhantomData }
	}

	/// Calling `into_inner` without flushing will lead to data loss
	pub fn into_inner(self) -> W {
		self.inner
	}

	pub fn into_parts(self) -> (W, Vec<u8>, usize) {
		(self.inner, self.buf, self.pos)
	}
}

#[async_trait_fn]
impl<Context: AsyncContext, W: Write<Context>> Write<Context> for BufWriter<Context, W> {
	/// Fully writes buf unless interrupt
	#[inline]
	async fn async_write(&mut self, buf: &[u8]) -> Result<usize> {
		if self.buf.spare_capacity_mut().len() >= buf.len() {
			return Ok(self.write_buffered(buf));
		}

		self.flush_buf().await?;

		if buf.len() < self.buf.capacity() {
			Ok(self.write_buffered(buf))
		} else if self.inner.write_all(buf).await? == buf.len() {
			Ok(buf.len())
		} else {
			Err(Error::new(
				ErrorKind::WriteZero,
				"Write returned EOF mid write"
			))
		}
	}

	async fn async_write_all(&mut self, buf: &[u8]) -> Result<usize> {
		self.write(buf).await
	}

	/// Flush buffer to stream
	///
	/// Upon interrupt, this function should be called again
	/// to finish flushing
	async fn async_flush(&mut self) -> Result<()> {
		self.flush_buf().await?;
		self.inner.flush().await?;

		Ok(())
	}
}

#[async_fn]
impl<Context: AsyncContext, W: Write<Context> + Seek<Context>> BufWriter<Context, W> {
	async fn seek_relative(&mut self, rel: i64) -> Result<u64> {
		let pos = rel.wrapping_add_unsigned(self.buf.len() as u64);

		/*
		 * as long as the seek is within our written buffer,
		 * we can fask seek
		 *
		 * otherwise, we'd have to fill in the blanks with the
		 * underlying stream data
		 */
		if pos >= 0 && pos as usize <= self.buf.len() {
			unsafe {
				self.buf.set_len(pos as usize);
			}

			self.stream_position().await
		} else {
			self.seek_inner(SeekFrom::Current(rel)).await
		}
	}

	async fn seek_inner(&mut self, seek: SeekFrom) -> Result<u64> {
		self.flush_buf().await?;

		let off = self.inner.seek(seek).await?;

		Ok(off)
	}
}

#[async_trait_fn]
impl<Context: AsyncContext, W: Write<Context> + Seek<Context>> Seek<Context>
	for BufWriter<Context, W>
{
	fn stream_len_fast(&self) -> bool {
		self.inner.stream_len_fast()
	}

	async fn async_stream_len(&mut self) -> Result<u64> {
		self.inner.stream_len().await
	}

	fn stream_position_fast(&self) -> bool {
		self.inner.stream_position_fast()
	}

	async fn async_stream_position(&mut self) -> Result<u64> {
		let pos = self.inner.stream_position().await?;
		let buffered = self.buf.len();

		Ok(pos + buffered as u64)
	}

	async fn async_seek(&mut self, seek: SeekFrom) -> Result<u64> {
		match seek {
			SeekFrom::Current(pos) => self.seek_relative(pos).await,
			SeekFrom::Start(pos) => {
				if !self.stream_position_fast() {
					return self.seek_inner(seek).await;
				}

				let stream_pos = self.stream_position().await?;

				self.seek_relative(pos.wrapping_sub(stream_pos) as i64)
					.await
			}

			SeekFrom::End(pos) => {
				if !self.stream_len_fast() || !self.stream_position_fast() {
					return self.seek_inner(seek).await;
				}

				let pos = self.stream_len().await?.checked_add_signed(pos).unwrap() as i64;
				let stream_pos = self.stream_position().await?;

				self.seek_relative(pos.wrapping_sub(stream_pos as i64))
					.await
			}
		}
	}
}

#[async_trait_fn]
impl<Context: AsyncContext, W: Write<Context> + Close<Context>> Close<Context>
	for BufWriter<Context, W>
{
	async fn async_close(self) -> Result<()> {
		self.inner.close().await
	}
}
