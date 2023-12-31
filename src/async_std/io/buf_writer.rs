use super::*;
use crate::impls::UIntExtensions;

pub struct BufWriter<W: Write> {
	inner: W,

	buf: Vec<u8>,
	pos: usize
}

#[async_fn]
impl<W: Write> BufWriter<W> {
	/// Discard all buffered data
	#[inline]
	fn discard(&mut self) {
		self.pos = 0;
		self.buf.clear();
	}

	/// Reads from `buf` into our internal buffer
	#[inline]
	unsafe fn write_buffered(&mut self, buf: &[u8]) -> usize {
		self.buf.extend_from_slice(buf);

		buf.len()
	}

	#[inline(never)]
	/// Flushes the buffer without flushing downstream
	async fn flush_buf(&mut self) -> Result<()> {
		while self.pos < self.buf.len() {
			let wrote = self.inner.write(&self.buf[self.pos..]).await?;

			if unlikely(wrote == 0) {
				return Err(Error::new(
					ErrorKind::WriteZero,
					"Write returned EOF while flushing"
				));
			}

			self.pos += wrote;
		}

		self.discard();

		Ok(())
	}

	pub fn new(inner: W) -> Self {
		Self::with_capacity(inner, DEFAULT_BUFFER_SIZE)
	}

	pub fn with_capacity(inner: W, capacity: usize) -> Self {
		Self::from_parts(inner, Vec::with_capacity(capacity), 0)
	}

	pub fn from_parts(inner: W, buf: Vec<u8>, pos: usize) -> Self {
		assert!(pos <= buf.len());

		BufWriter { inner, buf, pos }
	}

	/// Calling `into_inner` without flushing will lead to data loss
	pub fn into_inner(self) -> W {
		self.inner
	}

	pub fn into_parts(self) -> (W, Vec<u8>, usize) {
		(self.inner, self.buf, self.pos)
	}
}

#[async_trait_impl]
impl<W: Write> Write for BufWriter<W> {
	#[inline]
	async fn write(&mut self, buf: &[u8]) -> Result<usize> {
		if self.buf.spare_capacity_mut().len() >= buf.len() {
			return Ok(unsafe { self.write_buffered(buf) });
		}

		self.flush_buf().await?;

		if buf.len() < self.buf.capacity() {
			Ok(unsafe { self.write_buffered(buf) })
		} else {
			self.inner.write(buf).await
		}
	}

	/// Flush buffer to stream
	///
	/// On interrupt, this function should be called again
	/// to finish flushing
	async fn flush(&mut self) -> Result<()> {
		self.flush_buf().await?;
		self.inner.flush().await?;

		Ok(())
	}
}

#[async_fn]
impl<W: Write + Seek> BufWriter<W> {
	async fn seek_relative(&mut self, rel: i64) -> Result<u64> {
		let pos = rel.checked_add_unsigned(self.buf.len() as u64).unwrap();

		/*
		 * as long as the seek is within our written buffer,
		 * we can fask seek
		 *
		 * otherwise, we'd have to fill in the blanks with the
		 * underlying stream data, if we want to seek within the entire buffer
		 */
		if pos >= 0 && pos as usize <= self.buf.len() {
			self.buf.truncate(pos as usize);
			self.stream_position().await
		} else {
			self.seek_inner(SeekFrom::Current(rel)).await
		}
	}

	async fn seek_inner(&mut self, seek: SeekFrom) -> Result<u64> {
		self.flush_buf().await?;
		self.inner.seek(seek).await
	}

	async fn seek_abs(&mut self, abs: u64, seek: SeekFrom) -> Result<u64> {
		let stream_pos = self.stream_position().await?;
		let (rel, overflow) = abs.overflowing_signed_difference(stream_pos);

		if !overflow {
			self.seek_relative(rel).await
		} else {
			self.seek_inner(seek).await
		}
	}
}

#[async_trait_impl]
impl<W: Write + Seek> Seek for BufWriter<W> {
	fn stream_len_fast(&self) -> bool {
		self.inner.stream_len_fast()
	}

	async fn stream_len(&mut self) -> Result<u64> {
		self.inner.stream_len().await
	}

	fn stream_position_fast(&self) -> bool {
		self.inner.stream_position_fast()
	}

	async fn stream_position(&mut self) -> Result<u64> {
		let pos = self.inner.stream_position().await?;
		let buffered = self.buf.len();

		Ok(pos.checked_add(buffered as u64).unwrap())
	}

	async fn seek(&mut self, seek: SeekFrom) -> Result<u64> {
		match seek {
			SeekFrom::Current(pos) => self.seek_relative(pos).await,
			SeekFrom::Start(pos) => {
				if self.stream_position_fast() {
					self.seek_abs(pos, seek).await
				} else {
					self.seek_inner(seek).await
				}
			}

			SeekFrom::End(pos) => {
				if self.stream_len_fast() && self.stream_position_fast() {
					let new_pos = self.stream_len().await?.checked_add_signed(pos).unwrap();

					self.seek_abs(new_pos, seek).await
				} else {
					self.seek_inner(seek).await
				}
			}
		}
	}
}
