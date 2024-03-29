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
	fn write_buffered(&mut self, buf: &[u8]) -> usize {
		assert!(buf.len() <= self.buf.spare_capacity_mut().len());

		self.buf.extend_from_slice(buf);

		buf.len()
	}

	/// Flushes the buffer without flushing downstream
	#[inline(never)]
	async fn flush_buf(&mut self) -> Result<()> {
		while self.pos < self.buf.len() {
			let buf = &self.buf[self.pos..];
			let wrote = self.inner.write(buf).await?;

			if unlikely(wrote == 0) {
				return Err(Core::WriteZero.as_err());
			}

			#[allow(clippy::arithmetic_side_effects)]
			(self.pos += length_check(buf, wrote));

			#[cfg(feature = "tracing")]
			crate::trace!(target: self, "## flush_buf: write(buf = &[u8; {}]) = Ok({})", buf.len(), wrote);
		}

		#[cfg(feature = "tracing")]
		crate::trace!(target: self, "## flush_buf: complete()");

		self.discard();

		Ok(())
	}

	pub fn new(inner: W) -> Self {
		Self::with_capacity(inner, DEFAULT_BUFFER_SIZE)
	}

	pub fn with_capacity(inner: W, capacity: usize) -> Self {
		Self::from_parts(inner, Vec::with_capacity(capacity), 0)
	}

	/// # Panics
	/// if `pos` > `buf.len()`
	pub fn from_parts(inner: W, buf: Vec<u8>, pos: usize) -> Self {
		assert!(pos <= buf.len());

		Self { inner, buf, pos }
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
			let wrote = self.write_buffered(buf);

			#[cfg(feature = "tracing")]
			crate::trace!(target: self, "## write(buf = &[u8; {}]) = Buffered({})", buf.len(), wrote);

			return Ok(wrote);
		}

		self.flush_buf().await?;

		if buf.len() >= self.buf.capacity() {
			let wrote = self.inner.write(buf).await?;

			#[cfg(feature = "tracing")]
			crate::trace!(target: self, "## write(buf = &[u8; {}]) = Direct({})", buf.len(), wrote);

			Ok(wrote)
		} else {
			let wrote = self.write_buffered(buf);

			#[cfg(feature = "tracing")]
			crate::trace!(target: self, "## write(buf = &[u8; {}]) = Buffered({})", buf.len(), wrote);

			Ok(wrote)
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
		#[allow(clippy::never_loop)]
		loop {
			let Some(pos) = rel.checked_add_unsigned(self.pos as u64) else {
				break;
			};

			/* wrap cannot happen due to limits of vec's len */
			#[allow(clippy::cast_possible_wrap)]
			if pos < self.pos as i64 || pos > self.buf.len() as i64 {
				break;
			}

			#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
			self.buf.truncate(pos as usize);

			return self.stream_position().await;
		}

		self.seek_inner(SeekFrom::Current(rel)).await
	}

	async fn seek_inner(&mut self, seek: SeekFrom) -> Result<u64> {
		self.flush_buf().await?;

		let pos = self.inner.seek(seek).await;

		#[cfg(feature = "tracing")]
		crate::trace!(target: self, "## seek_inner(seek = {:?}) = {:?}", seek, pos);

		pos
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

		#[allow(clippy::expect_used)]
		Ok(pos
			.checked_add(buffered as u64)
			.expect("Overflow occurred calculating stream position"))
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
					#[allow(clippy::expect_used)]
					let new_pos = self
						.stream_len()
						.await?
						.checked_add_signed(pos)
						.expect("Overflow occured calculating absolute offset");

					self.seek_abs(new_pos, seek).await
				} else {
					self.seek_inner(seek).await
				}
			}
		}
	}
}
