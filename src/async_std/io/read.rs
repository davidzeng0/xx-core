use memchr::memchr;

use super::*;

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

#[asynchronous]
pub trait Read {
	/// Read into `buf`, returning the amount of bytes read
	///
	/// Returning zero strictly means EOF, unless the buffer's size was zero
	async fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

	/// Read until the buffer is filled, an I/O error, an interrupt, or an EOF
	///
	/// On interrupted, returns the number of bytes read if it is not zero
	async fn read_fully(&mut self, buf: &mut [u8]) -> Result<usize> {
		read_into!(buf);

		let mut read = 0;

		while read < buf.len() {
			let available = &mut buf[read..];

			match self.read(available).await {
				Ok(0) => break,
				Ok(n) => read += length_check(available, n),
				Err(err) if err.is_interrupted() => break,
				Err(err) => return Err(err)
			}
		}

		check_interrupt_if_zero(read).await
	}

	/// Same as above, except returns err on partial reads, even
	/// when interrupted
	async fn read_exact(&mut self, buf: &mut [u8]) -> Result<usize> {
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
	async fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
		let start_len = buf.len();

		loop {
			if is_interrupted().await {
				/* avoid doubling the capacity if we're interrupted */
				break;
			}

			let mut capacity = buf.capacity();
			let len = buf.len();

			if len == capacity {
				buf.reserve(32);
				capacity = buf.capacity();
			}

			unsafe {
				let available = buf.get_unchecked_mut(len..capacity);

				match self.read(available).await {
					Ok(0) => break,
					Ok(n) => {
						let new_len = len + length_check(available, n);

						buf.set_len(new_len)
					}

					Err(err) if err.is_interrupted() => break,
					Err(err) => return Err(err)
				}
			}

			if buf.len() == capacity {
				/* avoid doubling the capacity if EOF. try probing for more data */
				let mut probe = [0u8; 32];

				match self.read(&mut probe).await {
					Ok(0) => break,
					Ok(n) => buf.extend_from_slice(&probe[0..n]),
					Err(err) if err.is_interrupted() => break,
					Err(err) => return Err(err)
				}
			}
		}

		check_interrupt_if_zero(buf.len() - start_len).await
	}

	async fn read_to_string(&mut self, buf: &mut String) -> Result<usize> {
		let vec = unsafe { buf.as_mut_vec() };
		let start_len = vec.len();

		let result = self.read_to_end(vec).await.and_then(|read| {
			check_utf8(&vec[start_len..])?;

			Ok(read)
		});

		if result.is_err() {
			vec.truncate(start_len);
		}

		return result;
	}

	fn is_read_vectored(&self) -> bool {
		false
	}

	async fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
		match bufs.iter_mut().find(|b| !b.is_empty()) {
			Some(buf) => self.read(&mut buf[..]).await,
			None => Ok(0)
		}
	}

	async fn read_all_vectored(&mut self, mut bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
		let mut total = 0;

		while bufs.len() > 0 {
			let read = match self.read_vectored(bufs).await {
				Ok(0) => break,
				Ok(n) => n,
				Err(err) if err.is_interrupted() => break,
				Err(err) => return Err(err)
			};

			total += read;

			advance_slices_mut(&mut bufs, read);
		}

		Ok(total)
	}
}

pub trait AsReadRef: Read {
	fn as_ref(&mut self) -> ReadRef<'_, Self> {
		ReadRef::new(self)
	}
}

impl<T: Read> AsReadRef for T {}

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
			async fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

			#[asynchronous(traitfn)]
			async fn read_fully(&mut self, buf: &mut [u8]) -> Result<usize>;

			#[asynchronous(traitfn)]
			async fn read_exact(&mut self, buf: &mut [u8]) -> Result<usize>;

			#[asynchronous(traitfn)]
			async fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize>;

			#[asynchronous(traitfn)]
			async fn read_to_string(&mut self, buf: &mut String) -> Result<usize>;

			#[asynchronous(traitfn)]
			fn is_read_vectored(&self) -> bool;

			#[asynchronous(traitfn)]
			async fn read_vectored(&mut self, bufs: &mut [std::io::IoSliceMut<'_>]) -> Result<usize>;
		}
	}
}

pub use read_wrapper;

pub struct ReadRef<'a, R: Read + ?Sized> {
	reader: &'a mut R
}

impl<'a, R: Read + ?Sized> ReadRef<'a, R> {
	pub fn new(reader: &'a mut R) -> Self {
		Self { reader }
	}
}

impl<'a, R: Read + ?Sized> Read for ReadRef<'a, R> {
	read_wrapper! {
		inner = reader;
		mut inner = reader;
	}
}

#[asynchronous]
pub trait BufRead: Read {
	/// Fill any remaining space in the internal buffer,
	/// up to `amount` total unconsumed bytes
	///
	/// Returns the number of additional bytes filled, which can be zero
	async fn fill_amount(&mut self, amount: usize) -> Result<usize>;

	#[inline(never)]
	async fn fill(&mut self) -> Result<usize> {
		self.fill_amount(self.capacity()).await
	}

	/// Read until `byte`
	///
	/// On interrupted, the read bytes can be calculated using the difference in
	/// length of `buf` and can be called again with a new slice
	async fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> Result<Option<usize>> {
		let start_len = buf.len();

		loop {
			let available = self.buffer();
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

		Ok(Some(buf.len() - start_len))
	}

	/// See `read_until`
	///
	/// On interrupt, this function cannot be called again because it may stop
	/// reading in the middle of a utf8 character
	async fn read_line(&mut self, buf: &mut String) -> Result<Option<usize>> {
		let vec = unsafe { buf.as_mut_vec() };
		let start_len = vec.len();

		let mut result = self.read_until(b'\n', vec).await;

		result = result.and_then(|read| match read {
			Some(read) => {
				check_utf8(&vec[start_len..])?;

				Ok(Some(read))
			}

			None => Ok(None)
		});

		match result {
			Ok(Some(_)) => {
				if buf.ends_with('\n') {
					buf.pop();

					if buf.ends_with('\r') {
						buf.pop();
					}
				}

				Ok(Some(buf.len() - start_len))
			}

			Ok(None) => Ok(None),
			Err(err) => {
				unsafe {
					vec.set_len(start_len);
				}

				Err(err)
			}
		}
	}

	fn capacity(&self) -> usize;

	fn spare_capacity(&self) -> usize;

	fn buffer(&self) -> &[u8];

	fn consume(&mut self, count: usize);

	unsafe fn consume_unchecked(&mut self, count: usize);

	fn unconsume(&mut self, count: usize);

	unsafe fn unconsume_unchecked(&mut self, count: usize);

	/// Discard all data in the buffer
	fn discard(&mut self);
}

pub trait IntoLines: BufRead {
	fn lines(self) -> Lines<Self>
	where
		Self: Sized
	{
		Lines::new(self)
	}
}

impl<T: BufRead> IntoLines for T {}

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
			async fn fill_amount(&mut self, amount: usize) -> Result<usize>;

			#[asynchronous(traitfn)]
			async fn fill(&mut self) -> Result<usize>;

			#[asynchronous(traitfn)]
			async fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> Result<Option<usize>>;

			#[asynchronous(traitfn)]
			async fn read_line(&mut self, buf: &mut String) -> Result<Option<usize>>;

			#[asynchronous(traitfn)]
			fn capacity(&self) -> usize;

			#[asynchronous(traitfn)]
			fn spare_capacity(&self) -> usize;

			#[asynchronous(traitfn)]
			fn buffer(&self) -> &[u8];

			#[asynchronous(traitfn)]
			fn consume(&mut self, count: usize);

			#[asynchronous(traitfn)]
			unsafe fn consume_unchecked(&mut self, count: usize);

			#[asynchronous(traitfn)]
			fn unconsume(&mut self, count: usize);

			#[asynchronous(traitfn)]
			unsafe fn unconsume_unchecked(&mut self, count: usize);

			#[asynchronous(traitfn)]
			fn discard(&mut self);
		}
	}
}

pub use bufread_wrapper;

impl<'a, R: BufRead + ?Sized> BufRead for ReadRef<'a, R> {
	bufread_wrapper! {
		inner = reader;
		mut inner = reader;
	}
}

pub struct Lines<R: BufRead> {
	reader: R
}

impl<R: BufRead> Lines<R> {
	pub fn new(reader: R) -> Self {
		Self { reader }
	}
}

#[asynchronous]
impl<R: BufRead> AsyncIterator for Lines<R> {
	type Item = Result<String>;

	async fn next(&mut self) -> Option<Self::Item> {
		let mut line = String::new();

		match self.reader.read_line(&mut line).await {
			Err(err) => Some(Err(err)),
			Ok(Some(_)) => Some(Ok(line)),
			Ok(None) => None
		}
	}
}
