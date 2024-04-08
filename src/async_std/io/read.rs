#![allow(clippy::module_name_repetitions)]

use memchr::memchr;

use super::*;
use crate::impls::AsyncFnOnce;

/// Read from `src` to `dest`
pub fn read_into_slice(dest: &mut [u8], src: &[u8]) -> usize {
	let len = dest.len().min(src.len());

	/* adding any checks for small lengths only worsens performance
	 * it seems like llvm or rustc can't do branching properly
	 * (unlikely branches should be placed at the end, but that doesn't happen)
	 *
	 * a call to memcpy should do those checks anyway
	 */
	dest[0..len].copy_from_slice(&src[0..len]);
	len
}

/// Appends to `buf` by calling `read` with the string's buffer
///
/// Resets the buffer if the bytes are not valid UTF-8 or on panic
#[asynchronous]
pub async fn append_to_string<F>(buf: &mut String, read: F) -> Result<Option<usize>>
where
	F: for<'a> AsyncFnOnce<&'a mut Vec<u8>, Output = Result<Option<usize>>>
{
	/* panic guard */
	struct Guard<'a> {
		buf: &'a mut Vec<u8>,
		len: usize
	}

	impl Drop for Guard<'_> {
		fn drop(&mut self) {
			self.buf.truncate(self.len);
		}
	}

	#[allow(unsafe_code)]
	let mut guard = Guard {
		len: buf.len(),
		/* Safety: we truncate if the utf8 check fails */
		buf: unsafe { buf.as_mut_vec() }
	};

	let read = read.call_once(guard.buf).await?;

	if read.is_some() {
		check_utf8(&guard.buf[guard.len..])?;

		guard.len = guard.buf.len();
	}

	Ok(read)
}

/// The async equivalent of [`std::io::Read`]
///
/// This trait is object safe
#[asynchronous]
pub trait Read {
	/// Read into `buf`, returning the amount of bytes read
	///
	/// Returns zero if the `buf` is empty, or if the stream reached EOF
	///
	/// See also [`std::io::Read::read`]
	async fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

	/// Read until the buffer is filled, an I/O error, an interrupt, or an EOF
	///
	/// On interrupted, returns the number of bytes read if it is not zero
	///
	/// See also [`std::io::Read::read_exact`]
	async fn read_fully(&mut self, buf: &mut [u8]) -> Result<usize> {
		read_into!(buf);

		let mut read = 0;

		while read < buf.len() {
			let available = &mut buf[read..];

			#[allow(clippy::arithmetic_side_effects)]
			match self.read(available).await {
				Ok(0) => break,
				Ok(n) => read += length_check(available, n),
				Err(err) if err.is_interrupted() => break,
				Err(err) => return Err(err)
			}
		}

		check_interrupt_if_zero(read).await
	}

	/// Same as [`read_fully`], except returns an [`UnexpectedEof`] error
	/// on partial reads Returns the number of bytes read, which is the same as
	/// `buf.len()`
	///
	/// See also [`std::io::Read::read_exact`]
	///
	/// [`read_fully`]: Read::read_fully
	/// [`UnexpectedEof`]: Core::UnexpectedEof
	async fn read_exact(&mut self, buf: &mut [u8]) -> Result<usize> {
		read_into!(buf);

		let read = self.read_fully(buf).await?;

		length_check(buf, read);

		if unlikely(read != buf.len()) {
			return Err(short_io_error_unless_interrupt().await);
		}

		Ok(read)
	}

	/// Reads until an EOF, I/O error, or interrupt
	///
	/// On interrupted, returns the number of bytes read if it is not zero
	///
	/// See also [`std::io::Read::read_to_end`]
	async fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
		let start_len = buf.len();
		let mut len = buf.len();

		loop {
			if is_interrupted().await {
				/* avoid doubling the capacity if we're interrupted */
				break;
			}

			let mut capacity = buf.capacity();

			if len == capacity {
				buf.reserve(32);
				capacity = buf.capacity();
				buf.resize(capacity, 0);
			}

			let available = &mut buf[len..];

			#[allow(clippy::arithmetic_side_effects)]
			match self.read(available).await {
				Ok(0) => break,
				Ok(n) => len += length_check(available, n),
				Err(err) if err.is_interrupted() => break,
				Err(err) => return Err(err)
			}

			if len < capacity {
				continue;
			}

			/* avoid doubling the capacity if EOF. try probing for more data */
			let mut probe = [0u8; 32];

			match self.read(&mut probe).await {
				Ok(0) => break,
				Ok(n) => {
					#[allow(clippy::arithmetic_side_effects)]
					(len += length_check(&probe, n));

					buf.extend_from_slice(&probe[0..n]);
					buf.resize(buf.capacity(), 0);
				}

				Err(err) if err.is_interrupted() => break,
				Err(err) => return Err(err)
			}
		}

		buf.truncate(len);

		#[allow(clippy::arithmetic_side_effects)]
		check_interrupt_if_zero(len - start_len).await
	}

	/// Reads until EOF, I/O error, or interrupt
	///
	/// On interrupt, returns the number of bytes read if it is not zero
	///
	/// Interrupts may cause the read operation to stop in the middle of a
	/// character, in which case an [`InvalidUtf8`] error may be returned
	///
	/// See also [`std::io::Read::read_to_string`]
	///
	/// [`InvalidUtf8`]: Core::InvalidUtf8
	async fn read_to_string(&mut self, buf: &mut String) -> Result<usize> {
		append_to_string(buf, |vec: &mut Vec<u8>| async move {
			self.read_to_end(vec).await.map(Some)
		})
		.await
		.map(Option::unwrap)
	}

	/// Returns `true` if this `Read` implementation has an efficient
	/// [`read_vectored`] implementation
	///
	/// See also [`std::io::Read::is_read_vectored`]
	///
	/// [`read_vectored`]: Read::read_vectored
	fn is_read_vectored(&self) -> bool {
		false
	}

	/// Like [`read`], except that it reads into a slice of buffers
	///
	/// See also [`std::io::Read::read_vectored`]
	///
	/// [`read`]: Read::read
	async fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
		match bufs.iter_mut().find(|b| !b.is_empty()) {
			Some(buf) => self.read(&mut buf[..]).await,
			None => Ok(0)
		}
	}

	/// Like [`read_vectored`], except that it keeps reading until all the
	/// buffers are filled
	///
	/// [`read_vectored`]: Read::read_vectored
	async fn read_all_vectored(&mut self, mut bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
		let mut total = 0;

		while !bufs.is_empty() {
			let read = match self.read_vectored(bufs).await {
				Ok(0) => break,
				Ok(n) => n,
				Err(err) if err.is_interrupted() => break,
				Err(err) => return Err(err)
			};

			advance_slices_mut(&mut bufs, read);

			/* checked by `advance_slices_mut` */
			#[allow(clippy::arithmetic_side_effects)]
			(total += read);
		}

		Ok(total)
	}
}

#[macro_export]
macro_rules! read_wrapper {
	{
		inner = $inner: expr;
		mut inner = $inner_mut: expr;
	} => {
		$crate::macros::wrapper_functions! {
			inner = self.$inner;
			mut inner = self.$inner_mut;

			#[asynchronous(traitfn)]
			async fn read(&mut self, buf: &mut [u8]) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn read_fully(&mut self, buf: &mut [u8]) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn read_exact(&mut self, buf: &mut [u8]) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn read_to_end(&mut self, buf: &mut Vec<u8>) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn read_to_string(&mut self, buf: &mut String) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			fn is_read_vectored(&self) -> bool;

			#[asynchronous(traitfn)]
			async fn read_vectored(&mut self, bufs: &mut [::std::io::IoSliceMut<'_>]) -> $crate::error::Result<usize>;
		}
	}
}

pub use read_wrapper;

/// The async equivalent of [`std::io::BufRead`]
///
/// This trait is object safe
#[asynchronous]
pub trait BufRead: Read {
	/// Fill any remaining space in the internal buffer
	/// up to `amount` total unconsumed bytes
	///
	/// Returns the number of additional bytes filled, which can be zero
	async fn fill_amount(&mut self, amount: usize) -> Result<usize>;

	/// Calls [`fill_amount`] with the [`capacity`]
	///
	/// [`fill_amount`]: BufRead::fill_amount
	/// [`capacity`]: BufRead::capacity
	async fn fill(&mut self) -> Result<usize> {
		self.fill_amount(self.capacity()).await
	}

	/// Reads all bytes into `buf` until the delimiter `byte`, or EOF
	///
	/// The `byte` is included in the `buf`
	/// Returns the number of bytes read
	///
	/// On interrupted, the read bytes can be calculated using the difference in
	/// length of `buf` and can be called again with a new slice
	async fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> Result<Option<usize>> {
		let start_len = buf.len();

		loop {
			let available = self.buffer();

			/* `index` < `len`, therefore `index + 1` <= `len` */
			#[allow(clippy::arithmetic_side_effects)]
			let (used, done) = match memchr(byte, available) {
				Some(index) => (index + 1, true),
				None => (available.len(), false)
			};

			buf.extend_from_slice(&available[0..used]);

			self.consume(used);

			if done {
				break;
			}

			if self.fill().await? == 0 {
				if buf.len() == start_len {
					return Ok(None);
				}

				break;
			}
		}

		#[allow(clippy::arithmetic_side_effects)]
		Ok(Some(buf.len() - start_len))
	}

	/// Reads all bytes into `buf` until a newline (0xA byte) or EOF, and strips
	/// the line ending, if any
	///
	/// On interrupt, this function cannot be called again because it may stop
	/// reading in the middle of a utf8 character, in which case it may return
	/// an [`InvalidUtf8`] error
	///
	/// Returns the number of bytes read, if any
	///
	/// See also [`read_until`]
	///
	/// [`InvalidUtf8`]: Core::InvalidUtf8
	///
	/// [`read_until`]: BufRead::read_until
	async fn read_line(&mut self, buf: &mut String) -> Result<Option<usize>> {
		let result = append_to_string(buf, |vec: &mut Vec<u8>| async move {
			self.read_until(b'\n', vec).await
		})
		.await;

		if buf.ends_with('\n') {
			buf.pop();

			if buf.ends_with('\r') {
				buf.pop();
			}
		}

		result
	}

	/// Returns the capacity of the internal buffer
	fn capacity(&self) -> usize;

	/// Returns the remaining space in the internal buffer
	fn spare_capacity(&self) -> usize;

	/// Returns a slice to the unconsumed buffered data
	fn buffer(&self) -> &[u8];

	/// Consumes `count` bytes from the buffer
	///
	/// # Panics
	/// If `count` is greater than the number of unconsumed bytes
	///
	/// See also [`std::io::BufRead::consume`]
	fn consume(&mut self, count: usize);

	/// Same as [`consume`], but unchecked
	///
	/// # Safety
	/// `count` must be within the valid range
	///
	/// [`consume`]: BufRead::consume
	#[allow(unsafe_code)]
	unsafe fn consume_unchecked(&mut self, count: usize);

	/// Unconsume `count` bytes from the buffer
	///
	/// The next call to [`Read::read`] will return the unconsumed bytes
	///
	/// # Panics
	/// If `count` is greater than the maximum number of bytes that can be
	/// unconsumed
	fn unconsume(&mut self, count: usize);

	/// Same as [`unconsume`], but unchecked
	///
	/// # Safety
	/// `count` must be within the valid range
	///
	/// [`unconsume`]: BufRead::unconsume
	#[allow(unsafe_code)]
	unsafe fn unconsume_unchecked(&mut self, count: usize);

	/// Discard all data in the buffer
	fn discard(&mut self);
}

pub trait IntoLines: BufReadSealed {
	/// Returns an iterator over the lines of this reader
	///
	/// See also [`std::io::BufRead::lines`]
	fn lines(self) -> Lines<Self>
	where
		Self: Sized
	{
		Lines::new(self)
	}
}

impl<T: BufReadSealed> IntoLines for T {}

#[macro_export]
macro_rules! bufread_wrapper {
	{
		inner = $inner: expr;
		mut inner = $inner_mut: expr;
	} => {
		$crate::macros::wrapper_functions! {
			inner = self.$inner;
			mut inner = self.$inner_mut;

			#[asynchronous(traitfn)]
			async fn fill_amount(&mut self, amount: usize) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn fill(&mut self) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> $crate::error::Result<Option<usize>>;

			#[asynchronous(traitfn)]
			async fn read_line(&mut self, buf: &mut String) -> $crate::error::Result<Option<usize>>;

			#[asynchronous(traitfn)]
			fn capacity(&self) -> usize;

			#[asynchronous(traitfn)]
			fn spare_capacity(&self) -> usize;

			#[asynchronous(traitfn)]
			fn buffer(&self) -> &[u8];

			#[asynchronous(traitfn)]
			fn consume(&mut self, count: usize);

			#[asynchronous(traitfn)]
			#[allow(unsafe_code)]
			unsafe fn consume_unchecked(&mut self, count: usize);

			#[asynchronous(traitfn)]
			fn unconsume(&mut self, count: usize);

			#[asynchronous(traitfn)]
			#[allow(unsafe_code)]
			unsafe fn unconsume_unchecked(&mut self, count: usize);

			#[asynchronous(traitfn)]
			fn discard(&mut self);
		}
	}
}

pub use bufread_wrapper;

pub struct Lines<R> {
	reader: R
}

impl<R: BufRead> Lines<R> {
	pub const fn new(reader: R) -> Self {
		Self { reader }
	}
}

#[asynchronous]
impl<R: BufRead> AsyncIterator for Lines<R> {
	type Item = Result<String>;

	async fn next(&mut self) -> Option<Self::Item> {
		let mut line = String::new();
		let read = self.reader.read_line(&mut line).await.transpose()?;

		Some(read.map(|_| line))
	}
}
