use std::{io::IoSliceMut, marker::PhantomData, ptr::copy_nonoverlapping, str::from_utf8};

use super::bytes::BytesEncoding;
use crate::{
	async_std::{ext::ext_func, AsyncIterator},
	coroutines::{
		async_fn, async_trait_fn, async_trait_impl,
		env::AsyncContext,
		runtime::{check_interrupt, is_interrupted}
	},
	error::{Error, ErrorKind, Result},
	opt::hint::unlikely,
	xx_core
};

#[inline(always)]
pub fn read_into_slice(dest: &mut [u8], src: &[u8]) -> usize {
	let len = dest.len().min(src.len());

	/* adding any checks for small lengths only worsens performance
	 * it seems like llvm can't do branching properly
	 *
	 * a call to memcpy should do those checks anyway
	 */
	unsafe {
		copy_nonoverlapping(src.as_ptr(), dest.as_mut_ptr(), len);
	}

	len
}

fn check_utf8(buf: &[u8]) -> Result<()> {
	if from_utf8(buf).is_ok() {
		Ok(())
	} else {
		Err(Error::new(
			ErrorKind::InvalidData,
			"invalid UTF-8 found in stream"
		))
	}
}

#[async_trait_fn]
pub trait Read<Context: AsyncContext> {
	/// Read into `buf`, returning the amount of bytes read
	///
	/// Returning zero strictly means EOF, unless the buffer's size was zero
	async fn async_read(&mut self, buf: &mut [u8]) -> Result<usize>;

	/// Read until the buffer is filled, an I/O error, an interrupt, or an EOF
	///
	/// On interrupted, returns the number of bytes read if it is not zero
	async fn async_read_exact(&mut self, buf: &mut [u8]) -> Result<usize> {
		let mut read = 0;

		while read < buf.len() && !is_interrupted().await {
			match self.read(&mut buf[read..]).await {
				Ok(0) => break,
				Ok(n) => read += n,
				Err(err) => {
					if err.is_interrupted() {
						break;
					}

					return Err(err);
				}
			}
		}

		if read == 0 {
			check_interrupt().await?;
		}

		Ok(read)
	}

	/// Reads until an EOF, I/O error, or interrupt
	///
	/// On interrupted, returns the number of bytes read if it is not zero
	async fn async_read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
		let start_len = buf.len();

		while !is_interrupted().await {
			let mut capacity = buf.capacity();
			let len = buf.len();

			if len == capacity {
				buf.reserve(32);
			}

			unsafe {
				capacity = buf.capacity();

				match self.read(buf.get_unchecked_mut(len..capacity)).await {
					Ok(0) => break,
					Ok(read) => buf.set_len(len + read),
					Err(err) => {
						if err.is_interrupted() {
							break;
						}

						return Err(err);
					}
				}
			}

			if buf.len() == capacity {
				let mut probe = [0u8; 32];

				match self.read(&mut probe).await {
					Ok(0) => break,
					Ok(read) => {
						buf.extend_from_slice(&probe[0..read]);
					}

					Err(err) => {
						if err.is_interrupted() {
							break;
						}

						return Err(err);
					}
				}
			}
		}

		let total = buf.len() - start_len;

		if total == 0 {
			check_interrupt().await?;
		}

		Ok(total)
	}

	async fn read_to_string(&mut self, buf: &mut String) -> Result<usize> {
		let vec = unsafe { buf.as_mut_vec() };
		let start_len = vec.len();

		match self.read_to_end(vec).await {
			Err(err) => {
				unsafe { vec.set_len(start_len) };

				Err(err)
			}

			Ok(read) => {
				check_utf8(&vec[start_len..])?;

				Ok(read)
			}
		}
	}

	fn is_read_vectored(&self) -> bool {
		false
	}

	async fn async_read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
		let buf = match bufs.iter_mut().find(|b| !b.is_empty()) {
			Some(buf) => buf,
			None => return Ok(0)
		};

		self.read(&mut **buf).await
	}
}

pub trait ReadExt<Context: AsyncContext>: Read<Context> {
	ext_func!(read(self: &mut Self, buf: &mut [u8]) -> Result<usize>);

	ext_func!(read_exact(self: &mut Self, buf: &mut [u8]) -> Result<usize>);

	ext_func!(read_to_end(self: &mut Self, buf: &mut Vec<u8>) -> Result<usize>);

	ext_func!(read_vectored(self: &mut Self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize>);
}

impl<Context: AsyncContext, T: ?Sized + Read<Context>> ReadExt<Context> for T {}

#[async_trait_fn]
pub trait BufRead<Context: AsyncContext>: Read<Context> + Sized {
	/// Fill any remaining space in the internal buffer
	///
	/// Returns the number of bytes filled, which can be zero
	async fn async_fill_to(&mut self, amount: usize) -> Result<usize>;

	/// Read until `byte`
	///
	/// On interrupted, the read bytes can be calculated using the difference in
	/// length of `buf` and can be called again with a new slice
	async fn async_read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> Result<Option<usize>>;

	/// See `read_until`
	///
	/// On interrupt, this function cannot be called again because it may stop
	/// reading in the middle of a utf8 character
	async fn async_read_line(&mut self, buf: &mut String) -> Result<Option<usize>> {
		let vec = unsafe { buf.as_mut_vec() };
		let start_len = vec.len();

		let mut result = self.read_until(b'\n', vec).await;

		result = result.and_then(|read| match read {
			None => Ok(None),
			Some(read) => {
				check_utf8(&vec[start_len..])?;

				Ok(Some(read))
			}
		});

		match result {
			Err(err) => {
				unsafe {
					vec.set_len(start_len);
				}

				return Err(err);
			}
			Ok(None) => return Ok(None),
			Ok(Some(_)) => ()
		}

		if buf.ends_with('\n') {
			buf.pop();

			if buf.ends_with('\r') {
				buf.pop();
			}
		}

		Ok(Some(buf.len() - start_len))
	}

	fn capacity(&self) -> usize;

	fn buffer(&self) -> &[u8];

	fn consume(&mut self, count: usize);

	unsafe fn consume_unchecked(&mut self, count: usize);

	fn lines(self) -> Lines<Context, Self> {
		Lines::new(self)
	}
}

struct BufReadExtras<'a, Context: AsyncContext, R: BufRead<Context>> {
	reader: &'a mut R,
	phantom: PhantomData<Context>
}

#[async_fn]
impl<'a, Context: AsyncContext, R: BufRead<Context>> BufReadExtras<'a, Context, R> {
	fn new(reader: &'a mut R) -> Self {
		Self { reader, phantom: PhantomData }
	}

	async fn check_eof() -> Error {
		check_interrupt().await.err().unwrap_or(Error::new(
			ErrorKind::UnexpectedEof,
			"EOF while reading an int"
		))
	}

	#[inline(always)]
	async fn read_bytes_slow<const N: usize>(&mut self) -> Result<[u8; N]> {
		let mut bytes = [0u8; N];

		if self.reader.read_exact(&mut bytes).await? == N {
			Ok(bytes)
		} else {
			Err(Self::check_eof().await)
		}
	}

	#[inline(always)]
	fn read_bytes<const N: usize>(&mut self) -> [u8; N] {
		/* this function call gets optimized to a single load instruction of size N */
		let mut bytes = [0u8; N];

		for i in 0..N {
			bytes[i] = unsafe { *self.reader.buffer().get_unchecked(i) };
		}

		unsafe {
			self.reader.consume_unchecked(N);
		}

		bytes
	}

	#[inline(always)]
	pub async fn read_le<T: BytesEncoding<N>, const N: usize>(&mut self) -> Result<T> {
		/* for some llvm or rustc reason, unlikely here does the actual job of
		 * likely, and nets a performance gain on x64
		 */
		if unlikely(self.reader.buffer().len() >= N) {
			Ok(T::from_bytes_le(self.read_bytes()))
		} else {
			Ok(T::from_bytes_le(self.read_bytes_slow().await?))
		}
	}

	#[inline(always)]
	pub async fn read_be<T: BytesEncoding<N>, const N: usize>(&mut self) -> Result<T> {
		if unlikely(self.reader.buffer().len() >= N) {
			Ok(T::from_bytes_be(self.read_bytes()))
		} else {
			Ok(T::from_bytes_be(self.read_bytes_slow().await?))
		}
	}
}

pub trait BufReadExt<Context: AsyncContext>: BufRead<Context> {
	ext_func!(fill_to(self: &mut Self, amount: usize) -> Result<usize>);

	#[async_trait_impl]
	async fn fill(&mut self) -> Result<usize> {
		self.fill_to(self.capacity()).await
	}

	ext_func!(read_until(self: &mut Self, byte: u8, buf: &mut Vec<u8>) -> Result<Option<usize>>);

	ext_func!(read_line(self: &mut Self, buf: &mut String) -> Result<Option<usize>>);

	/// Read a number encoded in little endian bytes
	#[async_trait_impl]
	async fn read_le<T: BytesEncoding<N>, const N: usize>(&mut self) -> Result<T> {
		BufReadExtras::new(self).read_le().await
	}

	/// Read a number encoded in big endian bytes
	#[async_trait_impl]
	async fn read_be<T: BytesEncoding<N>, const N: usize>(&mut self) -> Result<T> {
		BufReadExtras::new(self).read_be().await
	}
}

impl<Context: AsyncContext, T: BufRead<Context>> BufReadExt<Context> for T {}

pub struct Lines<Context: AsyncContext, R: BufRead<Context>> {
	reader: R,
	phantom: PhantomData<Context>
}

impl<Context: AsyncContext, R: BufRead<Context>> Lines<Context, R> {
	pub fn new(reader: R) -> Self {
		Self { reader, phantom: PhantomData }
	}
}

#[async_trait_fn]
impl<Context: AsyncContext, R: BufRead<Context>> AsyncIterator<Context> for Lines<Context, R> {
	type Item = Result<String>;

	async fn async_next(&mut self) -> Option<Self::Item> {
		let mut line = String::new();

		match self.reader.read_line(&mut line).await {
			Err(err) => Some(Err(err)),
			Ok(Some(_)) => Some(Ok(line)),
			Ok(None) => None
		}
	}
}
