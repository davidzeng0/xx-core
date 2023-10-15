use std::{
	io::{Error, ErrorKind, Read as IoRead, Result, SeekFrom},
	marker::PhantomData,
	str::from_utf8
};

use memchr::memchr;

use super::{Close, Lines, Read, Seek, Stream};
use crate::{
	async_std::Iterator,
	coroutines::{async_fn, async_trait_fn, env::AsyncContext},
	xx_core
};

pub struct BufReader<Context: AsyncContext, R: Read<Context>> {
	inner: Stream<Context, R>,

	buf: Vec<u8>,
	pos: usize,

	phantom: PhantomData<Context>
}

#[async_fn]
impl<Context: AsyncContext, R: Read<Context>> BufReader<Context, R> {
	fn discard(&mut self) {
		self.pos = 0;

		unsafe {
			self.buf.set_len(0);
		}
	}

	fn read_into(&mut self, buf: &mut [u8]) -> Result<usize> {
		let read = (&self.buf[self.pos..]).read(buf)?;

		self.pos += read;

		Ok(read)
	}

	/* buf must be empty. fills from the start.
	 * if zero is returned, internal data is not modified
	 */
	async fn fill_buf(&mut self) -> Result<usize> {
		let capacity = self.buf.capacity();
		let read;

		unsafe {
			read = self
				.inner
				.read(self.buf.get_unchecked_mut(0..capacity))
				.await?;

			if read != 0 {
				self.pos = 0;
				self.buf.set_len(read);
			}
		}

		Ok(read)
	}

	pub fn new(inner: R) -> Stream<Context, Self> {
		Self::with_capacity(inner, 16384)
	}

	pub fn with_capacity(inner: R, capacity: usize) -> Stream<Context, Self> {
		Stream::new(BufReader {
			inner: Stream::new(inner),

			buf: Vec::with_capacity(capacity),
			pos: 0,

			phantom: PhantomData
		})
	}

	pub fn into_inner(self) -> R {
		self.inner.into_inner()
	}

	pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
		if self.buf.len() - self.pos > 0 {
			return self.read_into(buf);
		}

		if buf.len() >= self.buf.capacity() {
			return self.inner.read(buf).await;
		}

		if self.fill_buf().await? == 0 {
			Ok(0)
		} else {
			self.read_into(buf)
		}
	}

	pub async fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> Result<Option<usize>> {
		let start_len = buf.len();

		loop {
			let available = &self.buf[self.pos..];

			let (used, done) = match memchr(byte, available) {
				Some(index) => (index + 1, true),
				None => (available.len(), false)
			};

			buf.extend_from_slice(&available[0..used]);

			self.pos += used;

			if done {
				break;
			}

			if self.fill_buf().await? == 0 {
				if buf.len() == start_len {
					return Ok(None);
				}

				break;
			}
		}

		Ok(Some(buf.len() - start_len))
	}

	pub async fn read_line(&mut self, buf: &mut String) -> Result<Option<usize>> {
		let vec = unsafe { buf.as_mut_vec() };
		let valid_len = vec.len();

		let mut result = self.read_until(b'\n', vec).await;

		result = result.and_then(|read| match read {
			None => Ok(None),
			Some(read) => {
				if let Err(_) = from_utf8(&vec[valid_len..]) {
					Err(Error::new(
						ErrorKind::InvalidData,
						"invalid UTF-8 found in stream"
					))
				} else {
					Ok(Some(read))
				}
			}
		});

		match result {
			Err(_) => unsafe {
				vec.set_len(valid_len);
			},
			Ok(None) => (),
			Ok(Some(_)) => {
				if buf.ends_with('\n') {
					buf.pop();

					if buf.ends_with('\r') {
						buf.pop();
					}
				}
			}
		}

		result
	}

	pub fn lines(self) -> Iterator<Context, Lines<Context, R>> {
		Lines::new(self)
	}
}

#[async_trait_fn]
impl<Context: AsyncContext, R: Read<Context>> Read<Context> for BufReader<Context, R> {
	async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
		self.read(buf).await
	}
}

#[async_fn]
impl<Context: AsyncContext, R: Read<Context> + Seek<Context>> BufReader<Context, R> {
	async fn seek_relative(&mut self, rel: i64) -> Result<u64> {
		let pos = rel.wrapping_add_unsigned(self.pos as u64);

		if pos >= 0 && pos as usize <= self.buf.len() {
			self.pos = pos as usize;

			self.stream_position().await
		} else {
			self.inner.seek(SeekFrom::Current(pos)).await
		}
	}

	async fn seek_inner(&mut self, seek: SeekFrom) -> Result<u64> {
		let off = self.inner.seek(seek).await?;

		self.discard();

		Ok(off)
	}

	fn stream_len_fast(&self) -> bool {
		self.inner.stream_len_fast()
	}

	async fn stream_len(&mut self) -> Result<u64> {
		self.inner.stream_len().await
	}

	fn stream_position_fast(&self) -> bool {
		self.inner.stream_position_fast()
	}

	pub async fn stream_position(&mut self) -> Result<u64> {
		let pos = self.inner.stream_position().await?;
		let remaining = self.buf.len() - self.pos;

		Ok(pos - remaining as u64)
	}

	pub async fn seek(&mut self, seek: SeekFrom) -> Result<u64> {
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
impl<Context: AsyncContext, R: Read<Context> + Seek<Context>> Seek<Context>
	for BufReader<Context, R>
{
	async fn seek(&mut self, seek: SeekFrom) -> Result<u64> {
		self.seek(seek).await
	}

	fn stream_len_fast(&self) -> bool {
		self.stream_len_fast()
	}

	async fn stream_len(&mut self) -> Result<u64> {
		self.stream_len().await
	}

	fn stream_position_fast(&self) -> bool {
		self.stream_position_fast()
	}

	async fn stream_position(&mut self) -> Result<u64> {
		self.stream_position().await
	}
}

#[async_fn]
impl<Context: AsyncContext, R: Read<Context> + Close<Context>> BufReader<Context, R> {
	async fn close(self) -> Result<()> {
		self.inner.close().await
	}
}

#[async_trait_fn]
impl<Context: AsyncContext, R: Read<Context> + Close<Context>> Close<Context>
	for BufReader<Context, R>
{
	async fn close(self) -> Result<()> {
		self.close().await
	}
}

pub struct BufWriter {}
