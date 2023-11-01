use memchr::memchr;

use super::*;
use crate::impls::UIntExtensions;

pub struct BufReader<R: Read> {
	inner: R,

	buf: Vec<u8>,
	pos: usize,

	phantom: PhantomData<Context>
}

#[async_fn]
impl<R: Read> BufReader<R> {
	/// Reads from our internal buffer into `buf`
	#[inline]
	fn read_into(&mut self, buf: &mut [u8]) -> usize {
		let len = read_into_slice(buf, self.buffer());

		self.pos += len;

		len
	}

	#[inline]
	async fn fill_buf_offset(&mut self, start: usize, end: usize) -> Result<usize> {
		Ok(unsafe {
			/* bounds checking done by callers */
			let spare = self.buf.get_unchecked_mut(start..end);
			let read = self.inner.read(spare).await?;

			if likely(read != 0) {
				self.buf.set_len(start + read);
				self.pos = start;
			}

			read
		})
	}

	/// Fills the internal buffer from the start
	/// If zero is returned, internal data is not modified
	#[inline]
	async fn fill_buf(&mut self) -> Result<usize> {
		self.fill_buf_offset(0, self.buf.capacity()).await
	}

	pub fn new(inner: R) -> Self {
		Self::with_capacity(inner, DEFAULT_BUFFER_SIZE)
	}

	pub fn with_capacity(inner: R, capacity: usize) -> Self {
		Self::from_parts(inner, Vec::with_capacity(capacity), 0)
	}

	pub fn from_parts(inner: R, buf: Vec<u8>, pos: usize) -> Self {
		assert!(pos <= buf.len());

		BufReader { inner, buf, pos, phantom: PhantomData }
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

	pub fn move_data_to_beginning(&mut self) {
		let len = self.buffer().len();

		unsafe {
			copy(self.buffer().as_ptr(), self.buf.as_mut_ptr(), len);

			self.buf.set_len(len);
		}

		self.pos = 0;
	}

	pub fn position(&self) -> usize {
		self.pos
	}
}

#[async_trait_impl]
impl<R: Read> Read for BufReader<R> {
	#[inline]
	async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
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

#[async_trait_impl]
impl<R: Read> BufRead for BufReader<R> {
	async fn fill_amount(&mut self, amount: usize) -> Result<usize> {
		let amount = amount.min(self.buf.capacity());
		let mut end = self.pos + amount;

		if unlikely(end <= self.buf.len()) {
			return Ok(0);
		}

		if unlikely(end > self.buf.capacity()) {
			end = self.buf.capacity();

			if self.pos != 0 {
				if self.buffer().len() == 0 {
					self.discard();
				} else {
					self.move_data_to_beginning();
				}
			} else if self.spare_capacity() == 0 {
				return Ok(0);
			}
		}

		self.fill_buf_offset(self.buf.len(), end).await
	}

	async fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> Result<Option<usize>> {
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

			if self.fill_buf().await? == 0 {
				if buf.len() == start_len {
					return Ok(None);
				}

				break;
			}
		}

		Ok(Some(buf.len() - start_len))
	}

	fn capacity(&self) -> usize {
		self.buf.capacity()
	}

	fn spare_capacity(&self) -> usize {
		self.buf.capacity() - self.buf.len()
	}

	fn buffer(&self) -> &[u8] {
		unsafe { self.buf.get_unchecked(self.pos..) }
	}

	fn buffer_mut(&mut self) -> &mut [u8] {
		unsafe { self.buf.get_unchecked_mut(self.pos..) }
	}

	fn consume(&mut self, count: usize) {
		let new_pos = self.pos.checked_add(count).unwrap();

		assert!(new_pos <= self.buf.len());

		self.pos = new_pos;
	}

	/// Discard all data in the buffer
	#[inline]
	fn discard(&mut self) {
		self.pos = 0;

		unsafe {
			self.buf.set_len(0);
		}
	}

	unsafe fn consume_unchecked(&mut self, count: usize) {
		self.pos = self.pos.wrapping_add(count);
	}
}

#[async_fn]
impl<R: Read + Seek> BufReader<R> {
	async fn seek_relative(&mut self, rel: i64) -> Result<u64> {
		let pos = rel.checked_add_unsigned(self.pos as u64).unwrap();

		if pos >= 0 && pos as usize <= self.buf.len() {
			self.pos = pos as usize;
			self.stream_position().await
		} else {
			self.seek_inner(SeekFrom::Current(pos)).await
		}
	}

	async fn seek_inner(&mut self, seek: SeekFrom) -> Result<u64> {
		let off = self.inner.seek(seek).await?;

		/* seek functions should not be retried on error,
		 * so it's okay to discard only after a successfull seek
		 */
		self.discard();

		Ok(off)
	}

	async fn seek_abs(&mut self, abs: u64, seek: SeekFrom) -> Result<u64> {
		let stream_pos = self.stream_position().await?;
		let (rel, overflow) = abs.overflowing_difference_signed(stream_pos);

		if !overflow {
			self.seek_relative(rel).await
		} else {
			self.seek_inner(seek).await
		}
	}
}

#[async_trait_impl]
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

		Ok(pos - remaining as u64)
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
