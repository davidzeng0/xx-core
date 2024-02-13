use super::*;
use crate::impls::UIntExtensions;

pub struct BufWriter<W: Write> {
	inner: W,

	buf: Vec<u8>,
	pos: usize
}

#[asynchronous]
impl<W: Write> BufWriter<W> {
	/// Discard all buffered data
	fn discard(&mut self) {
		self.pos = 0;
		self.buf.clear();
	}

	/// Reads from `buf` into our internal buffer
	///
	/// Safety: buf len must not exceed spare capacity. there is nothing unsafe
	/// about this, but we don't want our buf to expand at all
	unsafe fn write_buffered(&mut self, buf: &[u8]) -> usize {
		self.buf.extend_from_slice(buf);

		buf.len()
	}

	/// Flushes the buffer without flushing downstream
	#[inline(never)]
	async fn flush_buf(&mut self) -> Result<()> {
		while self.pos < self.buf.len() {
			let buf = &self.buf[self.pos..];
			let wrote = self.inner.write(buf).await?;

			length_check(buf, wrote);

			if unlikely(wrote == 0) {
				return Err(Core::WriteZero.new());
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
		debug_assert!(pos <= buf.len());

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

#[asynchronous]
impl<W: Write> Write for BufWriter<W> {
	async fn write(&mut self, buf: &[u8]) -> Result<usize> {
		if self.buf.spare_capacity_mut().len() >= buf.len() {
			/* Safety: we just checked */
			return Ok(unsafe { self.write_buffered(buf) });
		}

		self.flush_buf().await?;

		if buf.len() >= self.buf.capacity() {
			self.inner.write(buf).await
		} else {
			/* Safety: we just checked */
			Ok(unsafe { self.write_buffered(buf) })
		}
	}

	/// Flush buffer to stream
	///
	/// On interrupt, this function should be called again
	/// to finish flushing
	async fn flush(&mut self) -> Result<()> {
		self.flush_buf().await?;
		self.inner.flush().await
	}
}

#[asynchronous]
impl<W: Write + Seek> BufWriter<W> {
	async fn seek_relative(&mut self, rel: i64) -> Result<u64> {
		let pos = rel
			.checked_add_unsigned(self.buf.len() as u64)
			.ok_or_else(|| Core::Overflow.new())?;

		if pos >= 0 && (self.pos..=self.buf.len()).contains(&(pos as usize)) {
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

#[asynchronous]
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

		pos.checked_add(buffered as u64)
			.ok_or_else(|| Core::Overflow.new())
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
					let new_pos = self
						.stream_len()
						.await?
						.checked_add_signed(pos)
						.ok_or_else(|| Core::Overflow.new())?;

					self.seek_abs(new_pos, seek).await
				} else {
					self.seek_inner(seek).await
				}
			}
		}
	}
}
