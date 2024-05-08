use super::*;
use crate::impls::UIntExtensions;

/// The async equivalent of [`std::io::BufWriter`]
pub struct BufWriter<W> {
	writer: W,
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

		#[cfg(feature = "tracing")]
		crate::trace!(target: &*self, "## write(buf = &[u8; {}]) = Buffered({})", buf.len(), buf.len());

		buf.len()
	}

	/// Flushes the buffer without flushing downstream
	#[inline(never)]
	#[cold]
	async fn flush_buf(&mut self) -> Result<()> {
		while self.pos < self.buf.len() {
			let buf = &self.buf[self.pos..];
			let wrote = self.writer.write(buf).await?;

			if wrote == 0 {
				return Err(Core::WriteZero.into());
			}

			#[allow(clippy::arithmetic_side_effects)]
			(self.pos += length_check(buf, wrote));

			#[cfg(feature = "tracing")]
			crate::trace!(target: &*self, "## flush_buf: write(buf = &[u8; {}]) = Ok({})", buf.len(), wrote);
		}

		self.discard();

		Ok(())
	}

	/// Creates a new `BufWriter<W>` with a [`DEFAULT_BUFFER_SIZE`]
	pub fn new(inner: W) -> Self {
		Self::with_capacity(inner, DEFAULT_BUFFER_SIZE)
	}

	/// Creates a new `BufWriter<W>` with the specified buffer capacity
	pub fn with_capacity(inner: W, capacity: usize) -> Self {
		Self::from_parts(inner, Vec::with_capacity(capacity), 0)
	}

	/// Creates a new `BufWriter<W>` from parts
	///
	/// # Panics
	/// If `pos` > `buf.len()`
	pub fn from_parts(writer: W, buf: Vec<u8>, pos: usize) -> Self {
		assert!(pos <= buf.len());

		Self { writer, buf, pos }
	}

	/// Unwraps this `BufWriter<W>`, returning the underlying writer
	///
	/// Any leftover data in the internal buffer is lost
	pub fn into_inner(self) -> W {
		self.writer
	}

	/// Unwraps this `BufWriter<W>`, returning its parts
	///
	/// The `Vec<u8>` contains the buffered data,
	/// and the `usize` is the position to start flushing
	pub fn into_parts(self) -> (W, Vec<u8>, usize) {
		(self.writer, self.buf, self.pos)
	}

	pub async fn write_from_once<R>(&mut self, reader: &mut R) -> Result<usize>
	where
		R: Read + ?Sized
	{
		if self.buf.spare_capacity_mut().is_empty() {
			self.flush().await?;
		}

		let mut amount = self.buf.len();

		self.buf.resize(self.buf.capacity(), 0);

		let buf = &mut self.buf[amount..];
		let result = reader.read(buf).await;

		if let Ok(read) = &result {
			#[allow(clippy::arithmetic_side_effects)]
			(amount += length_check(buf, *read));
		}

		self.buf.truncate(amount);

		result
	}

	pub async fn write_from<R>(&mut self, reader: &mut R) -> Result<usize>
	where
		R: Read + ?Sized
	{
		let mut total = 0;

		loop {
			#[allow(clippy::arithmetic_side_effects)]
			match self.write_from_once(reader).await {
				Ok(0) => break,
				Ok(n) => total += n,
				Err(err) if err.is_interrupted() => break,
				Err(err) => return Err(err)
			}
		}

		Ok(total)
	}
}

#[asynchronous]
impl<W: Write> Write for BufWriter<W> {
	async fn write(&mut self, buf: &[u8]) -> Result<usize> {
		if self.buf.spare_capacity_mut().len() >= buf.len() {
			return Ok(self.write_buffered(buf));
		}

		self.flush_buf().await?;

		Ok(if buf.len() >= self.buf.capacity() {
			let wrote = self.writer.write(buf).await?;

			#[cfg(feature = "tracing")]
			crate::trace!(target: &*self, "## write(buf = &[u8; {}]) = Direct({})", buf.len(), wrote);

			#[allow(clippy::let_and_return)]
			wrote
		} else {
			self.write_buffered(buf)
		})
	}

	/// Flush buffer to stream
	///
	/// On interrupt, this function should be called again
	/// to finish flushing
	async fn flush(&mut self) -> Result<()> {
		self.flush_buf().await?;
		self.writer.flush().await
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

		let pos = self.writer.seek(seek).await;

		#[cfg(feature = "tracing")]
		crate::trace!(target: &*self, "## seek_inner(seek = {:?}) = {:?}", seek, pos);

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
		self.writer.stream_len_fast()
	}

	async fn stream_len(&mut self) -> Result<u64> {
		self.writer.stream_len().await
	}

	fn stream_position_fast(&self) -> bool {
		self.writer.stream_position_fast()
	}

	/// # Panics
	/// If there was an overflow calculating the stream position
	async fn stream_position(&mut self) -> Result<u64> {
		let pos = self.writer.stream_position().await?;
		let buffered = self.buf.len();

		Ok(pos
			.checked_add(buffered as u64)
			.expect("Overflow occurred calculating stream position"))
	}

	/// # Panics
	/// If there was an overflow calculating the new position
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
						.expect("Overflow occured calculating absolute offset");

					self.seek_abs(new_pos, seek).await
				} else {
					self.seek_inner(seek).await
				}
			}
		}
	}
}
