use std::{io::SeekFrom, marker::PhantomData};

use memchr::memchr;

use super::{BufRead, Close, CloseExt, Read, ReadExt, Seek, SeekExt};
use crate::{
	coroutines::{async_fn, async_trait_fn, env::AsyncContext, runtime::check_interrupt},
	error::Result,
	opt::hint::likely,
	xx_core
};

pub struct BufReader<Context: AsyncContext, R: Read<Context>> {
	inner: R,

	buf: Vec<u8>,
	pos: usize,

	seek_threshold: u64,
	phantom: PhantomData<Context>
}

#[async_fn]
impl<Context: AsyncContext, R: Read<Context>> BufReader<Context, R> {
	/// Discard all data in the buffer
	#[inline(always)]
	fn discard(&mut self) {
		self.pos = 0;

		unsafe {
			self.buf.set_len(0);
		}
	}

	/// Reads from our internal buffer into `buf`
	#[inline(always)]
	fn read_into(&mut self, mut buf: &mut [u8]) -> usize {
		let mut src = unsafe { self.buf.get_unchecked(self.pos..) };
		let len = buf.len().min(src.len());

		self.pos += len;

		src = unsafe { src.get_unchecked(0..len) };
		buf = unsafe { buf.get_unchecked_mut(0..len) };
		buf.copy_from_slice(src);
		len
	}

	/// Fills the internal buffer from the start
	/// If zero is returned, internal data is not modified
	#[inline]
	async fn fill_buf(&mut self) -> Result<usize> {
		let capacity = self.buf.capacity();
		let read;

		unsafe {
			read = self
				.inner
				.read(self.buf.get_unchecked_mut(0..capacity))
				.await?;

			if likely(read != 0) {
				self.pos = 0;
				self.buf.set_len(read);
			}
		}

		Ok(read)
	}

	pub fn new(inner: R) -> Self {
		Self::with_capacity(inner, 16384)
	}

	pub fn with_capacity(inner: R, capacity: usize) -> Self {
		BufReader {
			inner,

			buf: Vec::with_capacity(capacity),
			pos: 0,

			seek_threshold: 0,
			phantom: PhantomData
		}
	}

	/// Calling `into_inner` with data in the buffer will lead to data loss
	pub fn into_inner(self) -> R {
		self.inner
	}

	/// If doing a relative seek forwards on a stream with
	/// an expensive seek operation
	///
	/// Prefer to read until that offset rather than seek if
	/// the difference <= threshold
	pub fn set_seek_threshold(&mut self, threshold: u64) {
		self.seek_threshold = threshold;
	}
}

#[async_trait_fn]
impl<Context: AsyncContext, R: Read<Context>> Read<Context> for BufReader<Context, R> {
	#[inline(always)]
	async fn async_read(&mut self, buf: &mut [u8]) -> Result<usize> {
		if likely(self.buf.len() != self.pos) {
			return Ok(self.read_into(buf));
		}

		if buf.len() >= self.buf.capacity() {
			return self.inner.read(buf).await;
		}

		if likely(self.fill_buf().await? != 0) {
			Ok(self.read_into(buf))
		} else {
			Ok(0)
		}
	}
}

#[async_trait_fn]
impl<Context: AsyncContext, R: Read<Context>> BufRead<Context> for BufReader<Context, R> {
	async fn async_read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> Result<Option<usize>> {
		let start_len = buf.len();

		loop {
			let available = self.buffer();

			let (used, done) = match memchr(byte, available) {
				Some(index) => (index + 1, true),
				None => (available.len(), false)
			};

			buf.extend_from_slice(unsafe { available.get_unchecked(0..used) });

			self.pos += used;

			if done {
				break;
			}

			check_interrupt().await?;

			if self.fill_buf().await? == 0 {
				if buf.len() == start_len {
					return Ok(None);
				}

				break;
			}
		}

		Ok(Some(buf.len() - start_len))
	}

	fn buffer(&self) -> &[u8] {
		unsafe { self.buf.get_unchecked(self.pos..) }
	}

	fn consume(&mut self, count: usize) {
		self.pos = self.buf.len().min(self.pos + count);
	}

	unsafe fn consume_unchecked(&mut self, count: usize) {
		self.pos += count;
	}
}

#[async_fn]
impl<Context: AsyncContext, R: Read<Context> + Seek<Context>> BufReader<Context, R> {
	async fn seek_relative(&mut self, rel: i64) -> Result<u64> {
		let pos = rel.wrapping_add_unsigned(self.pos as u64);

		if pos >= 0 && pos as usize <= self.buf.len() {
			self.pos = pos as usize;
			self.stream_position().await
		} else if pos > 0 && pos as u64 <= self.seek_threshold {
			let mut pos = pos as usize;

			self.discard();

			while pos > 0 {
				let len = pos.min(self.buf.capacity());
				let buf = unsafe { self.buf.get_unchecked_mut(0..len) };

				pos -= self.inner.read(buf).await?;
			}

			self.stream_position().await
		} else {
			self.seek_inner(SeekFrom::Current(pos)).await
		}
	}

	async fn seek_inner(&mut self, seek: SeekFrom) -> Result<u64> {
		let off = self.inner.seek(seek).await?;

		self.discard();

		Ok(off)
	}
}

#[async_trait_fn]
impl<Context: AsyncContext, R: Read<Context> + Seek<Context>> Seek<Context>
	for BufReader<Context, R>
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
		let remaining = self.buf.len() - self.pos;

		Ok(pos - remaining as u64)
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
impl<Context: AsyncContext, R: Read<Context> + Close<Context>> Close<Context>
	for BufReader<Context, R>
{
	async fn async_close(self) -> Result<()> {
		self.inner.close().await
	}
}
