use std::ops::Range;

use super::*;
use crate::impls::UIntExtensions;

pub struct BufReader<R: Read> {
	inner: R,

	buf: Vec<u8>,
	pos: usize
}

#[asynchronous]
impl<R: Read> BufReader<R> {
	/// Reads from our internal buffer into `buf`
	fn read_into(&mut self, buf: &mut [u8]) -> usize {
		let len = read_into_slice(buf, self.buffer());

		self.pos += len;

		len
	}

	/// Safety: the `range` must be within the buffer's capacity
	async unsafe fn fill_buf_range(&mut self, range: Range<usize>) -> Result<usize> {
		/* Safety: the contract is upheld by the caller */
		let buf = unsafe { self.buf.get_unchecked_mut(range.clone()) };
		let read = self.inner.read(buf).await?;

		if likely(read != 0) {
			let new_len = range.start + length_check(buf, read);

			self.buf.set_len(new_len);
		}

		Ok(read)
	}

	#[inline(never)]
	async fn fill_buf(&mut self) -> Result<usize> {
		/* Safety: the contract is met */
		let read = unsafe { self.fill_buf_range(0..self.buf.capacity()).await? };

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

	pub fn from_parts(inner: R, buf: Vec<u8>, pos: usize) -> Self {
		assert!(pos <= buf.len());

		#[cfg(any(test, feature = "test"))]
		let buf = {
			let mut buf = buf;

			/* valgrind doesn't like uninitialized data */
			for b in buf.spare_capacity_mut() {
				b.write(0);
			}

			buf
		};

		BufReader { inner, buf, pos }
	}

	/// Calling `into_inner` with data in the buffer will lead to data loss
	pub fn into_inner(self) -> R {
		self.inner
	}

	pub fn inner(&mut self) -> &mut R {
		&mut self.inner
	}

	pub fn into_parts(self) -> (R, Vec<u8>, usize) {
		(self.inner, self.buf, self.pos)
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
			self.buf.copy_within(self.pos.., 0);
			self.buf.truncate(len);
			self.pos = 0;
		}
	}

	/// The read head
	pub fn position(&self) -> usize {
		self.pos
	}
}

#[asynchronous]
impl<R: Read> Read for BufReader<R> {
	async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
		if likely(self.buffer().len() > 0) {
			return Ok(self.read_into(buf));
		}

		if buf.len() >= self.buf.capacity() {
			return self.inner.read(buf).await;
		}

		if unlikely(self.fill_buf().await? == 0) {
			return Ok(0);
		}

		Ok(self.read_into(buf))
	}
}

#[asynchronous]
impl<R: Read> BufRead for BufReader<R> {
	async fn fill_amount(&mut self, amount: usize) -> Result<usize> {
		assert!(amount <= self.buf.capacity());

		let mut start = self.buf.len();
		let mut end = self.pos + amount;

		if unlikely(end <= start) {
			return Ok(0);
		}

		if unlikely(end > self.buf.capacity()) {
			end = amount;

			if self.buffer().len() == 0 {
				/* try not to discard existing data if read returns EOF, assuming the read
				 * impl doesn't write junk even when returning zero */
				start = 0;
			} else {
				self.move_data_to_beginning();

				if self.spare_capacity() == 0 {
					return Ok(0);
				}

				start = self.buf.len();
			}
		}

		/* Safety: start <= buf.capacity() and end <= buf.capacity() */
		let read = unsafe { self.fill_buf_range(start..end).await? };

		if unlikely(start == 0 && read != 0) {
			/* read new data at beginning, reset pos */
			self.pos = 0;
		}

		Ok(read)
	}

	fn capacity(&self) -> usize {
		self.buf.capacity()
	}

	fn spare_capacity(&self) -> usize {
		self.buf.capacity() - self.buf.len()
	}

	fn buffer(&self) -> &[u8] {
		/* Safety: pos always <= self.buf.len() */
		unsafe { self.buf.get_unchecked(self.pos..) }
	}

	fn consume(&mut self, count: usize) {
		assert!(count <= self.buffer().len());

		self.pos += count;
	}

	unsafe fn consume_unchecked(&mut self, count: usize) {
		self.pos = self.pos.wrapping_add(count);
	}

	fn unconsume(&mut self, count: usize) {
		assert!(count <= self.pos);

		self.pos -= count;
	}

	unsafe fn unconsume_unchecked(&mut self, count: usize) {
		self.pos = self.pos.wrapping_sub(count);
	}

	fn discard(&mut self) {
		self.pos = 0;
		self.buf.clear();
	}
}

#[asynchronous]
impl<R: Read + Seek> BufReader<R> {
	async fn seek_relative(&mut self, rel: i64) -> Result<u64> {
		let pos = rel
			.checked_add_unsigned(self.pos as u64)
			.ok_or_else(|| Core::Overflow.as_err())?;

		if pos >= 0 && pos as usize <= self.buf.len() {
			self.pos = pos as usize;
			self.stream_position().await
		} else {
			self.seek_inner(SeekFrom::Current(rel)).await
		}
	}

	async fn seek_inner(&mut self, mut seek: SeekFrom) -> Result<u64> {
		if let SeekFrom::Current(pos) = &mut seek {
			let remainder = self.buffer().len();

			*pos = pos
				.checked_sub(remainder as i64)
				.ok_or_else(|| Core::Overflow.as_err())?;
		}

		self.discard();
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

		pos.checked_sub(remaining as u64)
			.ok_or_else(|| Core::Overflow.as_err())
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
						.ok_or_else(|| Core::Overflow.as_err())?;

					self.seek_abs(new_pos, seek).await
				} else {
					self.seek_inner(seek).await
				}
			}
		}
	}
}
