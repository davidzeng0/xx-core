use std::ops::Range;

use super::*;
use crate::impls::UIntExtensions;

/// The async equivalent of [`std::io::BufWriter`]
pub struct BufWriter<W: ?Sized> {
	data: Box<[u8]>,
	buffered: Range<usize>,
	writer: W
}

#[asynchronous]
impl<W: Write + ?Sized> BufWriter<W> {
	/// Discard all buffered data
	fn discard(&mut self) {
		self.buffered = 0..0;
	}

	#[allow(clippy::arithmetic_side_effects)]
	const fn spare_capacity(&self) -> usize {
		self.data.len() - self.buffered.end
	}

	/// Reads from `buf` into our internal buffer
	fn write_buffered(&mut self, buf: &[u8]) -> usize {
		let read = read_into_slice(&mut self.data[self.buffered.end..], buf);

		#[allow(clippy::arithmetic_side_effects)]
		(self.buffered.end += read);

		#[cfg(feature = "tracing")]
		crate::trace!(target: &*self, "## write(buf = &[u8; {}]) = Buffered({})", buf.len(), buf.len());

		read
	}

	/// Flushes the buffer without flushing downstream
	#[cold]
	async fn flush_buf(&mut self) -> Result<()> {
		while !self.buffered.is_empty() {
			let buf = &self.data[self.buffered.clone()];
			let wrote = self.writer.write(buf).await?;

			if wrote == 0 {
				return Err(Core::WriteZero.into());
			}

			#[allow(clippy::arithmetic_side_effects)]
			(self.buffered.start += length_check(buf, wrote));

			#[cfg(feature = "tracing")]
			crate::trace!(target: &*self, "## flush_buf: write(buf = &[u8; {}]) = Ok({})", buf.len(), wrote);
		}

		self.discard();

		Ok(())
	}

	/// Creates a new `BufWriter<W>` with a [`DEFAULT_BUFFER_SIZE`]
	pub fn new(inner: W) -> Self
	where
		W: Sized
	{
		Self::with_capacity(inner, DEFAULT_BUFFER_SIZE)
	}

	/// Creates a new `BufWriter<W>` with the specified buffer capacity
	pub fn with_capacity(inner: W, capacity: usize) -> Self
	where
		W: Sized
	{
		Self::from_parts(inner, Vec::with_capacity(capacity), 0)
	}

	/// Creates a new `BufWriter<W>` from parts
	///
	/// # Panics
	/// If `pos` > `buf.len()`
	pub fn from_parts(writer: W, mut buf: Vec<u8>, pos: usize) -> Self
	where
		W: Sized
	{
		let len = buf.len();

		assert!(pos <= len);

		buf.resize(buf.capacity(), 0);

		Self {
			writer,
			data: buf.into_boxed_slice(),
			buffered: pos..len
		}
	}

	/// Unwraps this `BufWriter<W>`, returning the underlying writer
	///
	/// Any leftover data in the internal buffer is lost
	pub fn into_inner(self) -> W
	where
		W: Sized
	{
		self.writer
	}

	/// Unwraps this `BufWriter<W>`, returning its parts
	///
	/// The `Vec<u8>` contains the buffered data,
	/// and the `usize` is the position to start flushing
	pub fn into_parts(self) -> (W, Vec<u8>, usize)
	where
		W: Sized
	{
		let mut buf = self.data.into_vec();

		buf.truncate(self.buffered.end);

		(self.writer, buf, self.buffered.start)
	}

	pub async fn write_from_once<R>(&mut self, reader: &mut R) -> Result<usize>
	where
		R: Read + ?Sized
	{
		if self.spare_capacity() == 0 {
			self.flush().await?;
		}

		let buf = &mut self.data[self.buffered.end..];
		let read = reader.read(buf).await?;

		#[allow(clippy::arithmetic_side_effects)]
		(self.buffered.end += length_check(buf, read));

		Ok(read)
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
impl<W: Write + ?Sized> Write for BufWriter<W> {
	async fn write(&mut self, buf: &[u8]) -> Result<usize> {
		if self.spare_capacity() > 0 {
			return Ok(self.write_buffered(buf));
		}

		self.flush_buf().await?;

		Ok(if buf.len() >= self.data.len() {
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
impl<W: Write + Seek + ?Sized> BufWriter<W> {
	async fn seek_relative(&mut self, rel: i64) -> Result<u64> {
		#[allow(clippy::never_loop)]
		loop {
			let Some(pos) = rel.checked_add_unsigned(self.buffered.start as u64) else {
				break;
			};

			/* wrap cannot happen due to limits of vec's len */
			#[allow(clippy::cast_possible_wrap)]
			if pos < self.buffered.start as i64 || pos > self.buffered.end as i64 {
				break;
			}

			#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
			(self.buffered.end = pos as usize);

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
		let (rel, overflow) = abs.overflowing_signed_diff(stream_pos);

		if !overflow {
			self.seek_relative(rel).await
		} else {
			self.seek_inner(seek).await
		}
	}
}

#[asynchronous]
impl<W: Write + Seek + ?Sized> Seek for BufWriter<W> {
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
		let buffered = self.data.len();

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
