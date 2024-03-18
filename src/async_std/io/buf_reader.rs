use std::ops::Range;

use super::*;
use crate::impls::UIntExtensions;

pub struct BufReader<R: Read> {
	inner: R,
	buf: Box<[u8]>,
	pos: usize,
	len: usize
}

#[asynchronous]
impl<R: Read> BufReader<R> {
	/// Reads from our internal buffer into `buf`
	fn read_into(&mut self, buf: &mut [u8]) -> usize {
		let len = read_into_slice(buf, self.buffer());

		#[allow(clippy::arithmetic_side_effects)]
		(self.pos += len);

		len
	}

	async fn fill_buf_range(&mut self, range: Range<usize>) -> Result<usize> {
		let buf = &mut self.buf[range.clone()];
		let read = self.inner.read(buf).await?;

		if likely(read != 0) {
			#[allow(clippy::arithmetic_side_effects)]
			(self.len = range.start + length_check(buf, read));
		}

		#[cfg(feature = "tracing")]
		crate::trace!(target: self, "## fill_buf_range(range = {:?}) = Ok({})", range, read);

		Ok(read)
	}

	#[inline(never)]
	async fn fill_buf(&mut self) -> Result<usize> {
		let read = self.fill_buf_range(0..self.buf.len()).await?;

		if likely(read != 0) {
			self.pos = 0;
		}

		Ok(read)
	}

	pub fn new(inner: R) -> Self {
		Self::with_capacity(inner, DEFAULT_BUFFER_SIZE)
	}

	pub fn with_capacity(inner: R, capacity: usize) -> Self {
		Self::from_parts(inner, Vec::with_capacity(capacity), 0)
	}

	/// # Panics
	/// if `pos` > `buf.len()`
	pub fn from_parts(inner: R, mut buf: Vec<u8>, pos: usize) -> Self {
		let len = buf.len();

		assert!(pos <= len);

		buf.resize(buf.capacity(), 0);

		Self { inner, buf: buf.into_boxed_slice(), pos, len }
	}

	/// Calling `into_inner` with data in the buffer will lead to data loss
	pub fn into_inner(self) -> R {
		self.inner
	}

	pub fn inner(&mut self) -> &mut R {
		&mut self.inner
	}

	pub fn into_parts(self) -> (R, Vec<u8>, usize) {
		let mut buf = self.buf.into_vec();

		buf.truncate(self.len);

		(self.inner, buf, self.pos)
	}

	/// Free up consumed bytes to fill more space without discarding
	pub fn move_data_to_beginning(&mut self) {
		if self.pos == 0 {
			return;
		}

		let len = self.buffer().len();

		if len == 0 {
			self.discard();
		} else {
			self.buf.copy_within(self.pos..self.len, 0);
			self.pos = 0;
			self.len = len;
		}
	}

	/// The read head
	pub const fn position(&self) -> usize {
		self.pos
	}
}

#[asynchronous]
impl<R: Read> Read for BufReader<R> {
	async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
		if likely(!self.buffer().is_empty()) {
			let read = self.read_into(buf);

			#[cfg(feature = "tracing")]
			crate::trace!(
				target: self,
				"## read(buf = &mut [u8; {}]) = Buffered({} / {})",
				buf.len(),
				read,
				{
					#[allow(clippy::arithmetic_side_effects)]
					(self.buffer().len() + read)
				}
			);

			return Ok(read);
		}

		if buf.len() >= self.capacity() {
			let read = self.inner.read(buf).await?;

			#[cfg(feature = "tracing")]
			crate::trace!(target: self, "## read(buf = &mut [u8; {}]) = Direct({})", buf.len(), read);

			return Ok(read);
		}

		if unlikely(self.fill_buf().await? == 0) {
			return Ok(0);
		}

		let read = self.read_into(buf);

		#[cfg(feature = "tracing")]
		crate::trace!(
			target: self,
			"## read(buf = &mut [u8; {}]) = Buffered({} / {})",
			buf.len(),
			read,
			{
				#[allow(clippy::arithmetic_side_effects)]
				(self.buffer().len() + read)
			}
		);

		Ok(read)
	}
}

#[asynchronous]
impl<R: Read> BufRead for BufReader<R> {
	async fn fill_amount(&mut self, amount: usize) -> Result<usize> {
		assert!(amount <= self.capacity());

		let mut start = self.len;

		/* cannot overflow here due to limits of buf's length */
		#[allow(clippy::arithmetic_side_effects)]
		let mut end = self.pos + amount;

		if unlikely(end <= start) {
			return Ok(0);
		}

		if unlikely(end > self.capacity()) {
			end = amount;

			if self.buffer().is_empty() {
				/* try not to discard existing data if read returns EOF, assuming the read
				 * impl doesn't write junk even when returning zero */
				start = 0;
			} else {
				self.move_data_to_beginning();

				if self.spare_capacity() == 0 {
					return Ok(0);
				}

				start = self.len;
			}
		}

		let read = self.fill_buf_range(start..end).await?;

		if unlikely(start == 0 && read != 0) {
			/* read new data at beginning, reset pos */
			self.pos = 0;
		}

		Ok(read)
	}

	fn capacity(&self) -> usize {
		self.buf.len()
	}

	fn spare_capacity(&self) -> usize {
		#[allow(clippy::arithmetic_side_effects)]
		(self.buf.len() - self.len)
	}

	fn buffer(&self) -> &[u8] {
		/* Safety: pos always <= self.len */
		unsafe { self.buf.get_unchecked(self.pos..self.len) }
	}

	#[allow(clippy::arithmetic_side_effects)]
	fn consume(&mut self, count: usize) {
		assert!(count <= self.buffer().len());

		self.pos += count;
	}

	unsafe fn consume_unchecked(&mut self, count: usize) {
		#[allow(clippy::arithmetic_side_effects)]
		(self.pos += count);
	}

	fn unconsume(&mut self, count: usize) {
		#[allow(clippy::expect_used)]
		(self.pos = self.pos.checked_sub(count).expect("`count` > `self.pos`"));
	}

	unsafe fn unconsume_unchecked(&mut self, count: usize) {
		#[allow(clippy::arithmetic_side_effects)]
		(self.pos -= count);
	}

	fn discard(&mut self) {
		self.pos = 0;
		self.len = 0;
	}
}

#[asynchronous]
impl<R: Read + Seek> BufReader<R> {
	async fn seek_relative(&mut self, rel: i64) -> Result<u64> {
		#[allow(clippy::never_loop)]
		loop {
			let Some(pos) = rel.checked_add_unsigned(self.pos as u64) else {
				break;
			};

			/* wrap cannot happen due to limits of buf's len */
			#[allow(clippy::cast_possible_wrap)]
			if pos < 0 || pos > self.len as i64 {
				break;
			}

			#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
			(self.pos = pos as usize);

			return self.stream_position().await;
		}

		self.seek_inner(SeekFrom::Current(rel)).await
	}

	async fn seek_inner(&mut self, mut seek: SeekFrom) -> Result<u64> {
		if let SeekFrom::Current(pos) = &mut seek {
			/* wrap cannot happen due to limits of buf's len */
			#[allow(clippy::cast_possible_wrap)]
			let remainder = self.buffer().len() as i64;

			if let Some(p) = pos.checked_sub(remainder) {
				*pos = p;
			} else {
				#[allow(clippy::arithmetic_side_effects)]
				self.inner.seek(SeekFrom::Current(-remainder)).await?;
			}
		}

		self.discard();

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
impl<R: Read + Seek> Seek for BufReader<R> {
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
		let remaining = self.buffer().len();

		#[allow(clippy::expect_used)]
		Ok(pos
			.checked_sub(remaining as u64)
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
