use std::{io::IoSliceMut, marker::PhantomData, ptr::copy_nonoverlapping};

use super::{check_interrupt_if_zero, check_utf8};
use crate::{
	async_std::{ext::ext_func, AsyncIterator},
	coroutines::*,
	error::*,
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

		while read < buf.len() {
			match self.read(&mut buf[read..]).await {
				Ok(0) => break,
				Ok(n) => read += n,
				Err(err) if err.is_interrupted() => break,
				Err(err) => return Err(err)
			}
		}

		check_interrupt_if_zero(read).await
	}

	async fn async_read_exact_or_err(&mut self, buf: &mut [u8]) -> Result<()> {
		let read = self.read_exact(buf).await?;

		if unlikely(read != buf.len()) {
			check_interrupt().await?;

			return Err(Error::new(ErrorKind::UnexpectedEof, "Short read"));
		}

		Ok(())
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
					Err(err) if err.is_interrupted() => break,
					Err(err) => return Err(err)
				}
			}

			if buf.len() == capacity {
				let mut probe = [0u8; 32];

				match self.read(&mut probe).await {
					Ok(0) => break,
					Ok(read) => {
						buf.extend_from_slice(&probe[0..read]);
					}

					Err(err) if err.is_interrupted() => break,
					Err(err) => return Err(err)
				}
			}
		}

		check_interrupt_if_zero(buf.len() - start_len).await
	}

	async fn async_read_to_string(&mut self, buf: &mut String) -> Result<usize> {
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
		match bufs.iter_mut().find(|b| !b.is_empty()) {
			Some(buf) => self.read(&mut **buf).await,
			None => Ok(0)
		}
	}
}

pub struct ReadRef<'a, Context: AsyncContext, R: Read<Context> + ?Sized> {
	reader: &'a mut R,
	phantom: PhantomData<Context>
}

impl<'a, Context: AsyncContext, R: Read<Context> + ?Sized> ReadRef<'a, Context, R> {
	pub fn new(reader: &'a mut R) -> Self {
		Self { reader, phantom: PhantomData }
	}
}

macro_rules! async_alias_func {
	($func: ident ($self: ident: $self_type: ty $(, $arg: ident: $type: ty)*) -> $return_type: ty) => {
		#[xx_core::coroutines::async_trait_fn]
		async fn $func($self: $self_type $(, $arg: $type)*) -> $return_type {
			$self.reader.$func($($arg,)* xx_core::coroutines::runtime::get_context().await)
		}
    }
}

macro_rules! alias_func {
	($func: ident ($self: ident: $self_type: ty $(, $arg: ident: $type: ty)*) -> $return_type: ty) => {
		fn $func($self: $self_type $(, $arg: $type)*) -> $return_type {
			$self.reader.$func($($arg,)*)
		}
    }
}

impl<Context: AsyncContext, R: Read<Context>> Read<Context> for ReadRef<'_, Context, R> {
	async_alias_func!(async_read(self: &mut Self, buf: &mut [u8]) -> Result<usize>);

	async_alias_func!(async_read_exact(self: &mut Self, buf: &mut [u8]) -> Result<usize>);

	async_alias_func!(async_read_exact_or_err(self: &mut Self, buf: &mut [u8]) -> Result<()>);

	async_alias_func!(async_read_to_end(self: &mut Self, buf: &mut Vec<u8>) -> Result<usize>);

	async_alias_func!(async_read_to_string(self: &mut Self, buf: &mut String) -> Result<usize>);

	async_alias_func!(async_read_vectored(self: &mut Self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize>);

	alias_func!(is_read_vectored(self: &Self) -> bool);
}

impl<Context: AsyncContext, R: BufRead<Context>> BufRead<Context> for ReadRef<'_, Context, R> {
	async_alias_func!(async_fill_amount(self: &mut Self, amount: usize) -> Result<usize>);

	async_alias_func!(async_read_until(self: &mut Self, byte: u8, buf: &mut Vec<u8>) -> Result<Option<usize>>);

	async_alias_func!(async_read_line(self: &mut Self, buf: &mut String) -> Result<Option<usize>>);

	alias_func!(capacity(self: &Self) -> usize);

	alias_func!(spare_capacity(self: &Self) -> usize);

	alias_func!(buffer(self: &Self) -> &[u8]);

	alias_func!(buffer_mut(self: &mut Self) -> &mut [u8]);

	alias_func!(consume(self: &mut Self, count: usize) -> ());

	alias_func!(discard(self: &mut Self) -> ());

	unsafe fn consume_unchecked(&mut self, count: usize) {
		self.reader.consume_unchecked(count)
	}
}

pub trait ReadExt<Context: AsyncContext>: Read<Context> {
	ext_func!(read(self: &mut Self, buf: &mut [u8]) -> Result<usize>);

	ext_func!(read_exact(self: &mut Self, buf: &mut [u8]) -> Result<usize>);

	ext_func!(read_exact_or_err(self: &mut Self, buf: &mut [u8]) -> Result<()>);

	ext_func!(read_to_end(self: &mut Self, buf: &mut Vec<u8>) -> Result<usize>);

	ext_func!(read_to_string(self: &mut Self, buf: &mut String) -> Result<usize>);

	ext_func!(read_vectored(self: &mut Self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize>);

	fn as_ref(&mut self) -> ReadRef<'_, Context, Self> {
		ReadRef::new(self)
	}
}

impl<Context: AsyncContext, T: ?Sized + Read<Context>> ReadExt<Context> for T {}

#[async_trait_fn]
pub trait BufRead<Context: AsyncContext>: Read<Context> + Sized {
	/// Fill any remaining space in the internal buffer,
	/// up to `amount` total unconsumed bytes
	///
	/// Returns the number of bytes filled, which can be zero
	async fn async_fill_amount(&mut self, amount: usize) -> Result<usize>;

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

	fn spare_capacity(&self) -> usize;

	fn buffer(&self) -> &[u8];

	fn buffer_mut(&mut self) -> &mut [u8];

	fn consume(&mut self, count: usize);

	fn discard(&mut self);

	unsafe fn consume_unchecked(&mut self, count: usize);

	fn lines(self) -> Lines<Context, Self> {
		Lines::new(self)
	}
}

pub trait BufReadExt<Context: AsyncContext>: BufRead<Context> {
	ext_func!(fill_amount(self: &mut Self, amount: usize) -> Result<usize>);

	#[async_trait_impl]
	async fn fill(&mut self) -> Result<usize> {
		self.fill_amount(self.capacity()).await
	}

	ext_func!(read_until(self: &mut Self, byte: u8, buf: &mut Vec<u8>) -> Result<Option<usize>>);

	ext_func!(read_line(self: &mut Self, buf: &mut String) -> Result<Option<usize>>);
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
