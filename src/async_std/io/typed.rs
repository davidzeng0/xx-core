use std::{
	fmt::{self, Arguments},
	marker::PhantomData,
	ops::{Deref, DerefMut}
};

use super::{BufRead, ReadExt, Write, WriteExt};
use crate::{coroutines::*, error::*, opt::hint::*, task::Handle, xx_core};

pub trait BytesEncoding<const N: usize> {
	fn to_bytes_le(self) -> [u8; N];
	fn to_bytes_be(self) -> [u8; N];

	fn from_bytes_le(bytes: [u8; N]) -> Self;
	fn from_bytes_be(bytes: [u8; N]) -> Self;
}

macro_rules! impl_bytes_type_bits {
	($type: ty, $bits: literal) => {
		impl BytesEncoding<{ $bits as usize / 8 }> for $type {
			#[inline(always)]
			fn to_bytes_le(self) -> [u8; $bits as usize / 8] {
				self.to_le_bytes()
			}

			#[inline(always)]
			fn to_bytes_be(self) -> [u8; $bits as usize / 8] {
				self.to_be_bytes()
			}

			#[inline(always)]
			fn from_bytes_le(bytes: [u8; $bits as usize / 8]) -> Self {
				Self::from_le_bytes(bytes)
			}

			#[inline(always)]
			fn from_bytes_be(bytes: [u8; $bits as usize / 8]) -> Self {
				Self::from_be_bytes(bytes)
			}
		}
	};
}

/* usize and isize omitted intentionally */
impl_bytes_type_bits!(i8, 8);
impl_bytes_type_bits!(u8, 8);
impl_bytes_type_bits!(i16, 16);
impl_bytes_type_bits!(u16, 16);
impl_bytes_type_bits!(i32, 32);
impl_bytes_type_bits!(u32, 32);
impl_bytes_type_bits!(i64, 64);
impl_bytes_type_bits!(u64, 64);
impl_bytes_type_bits!(i128, 128);
impl_bytes_type_bits!(u128, 128);
impl_bytes_type_bits!(f32, 32);
impl_bytes_type_bits!(f64, 64);

pub struct TypedReader<Context: AsyncContext, R: BufRead<Context>> {
	reader: R,
	phantom: PhantomData<Context>
}

fn eof_error() -> Error {
	Error::new(ErrorKind::UnexpectedEof, "Unexpected end of stream")
}

#[async_fn]
async fn check_eof<Context: AsyncContext>() -> Error {
	check_interrupt().await.err().unwrap_or(eof_error())
}

#[async_fn]
impl<Context: AsyncContext, R: BufRead<Context>> TypedReader<Context, R> {
	pub fn new(reader: R) -> Self {
		Self { reader, phantom: PhantomData }
	}

	pub fn into_inner(self) -> R {
		self.reader
	}

	#[inline(always)]
	async fn read_bytes_slow<const N: usize>(&mut self) -> Result<Option<[u8; N]>> {
		let mut bytes = [0u8; N];
		let read = self.reader.read_exact(&mut bytes).await?;

		if read == N {
			Ok(Some(bytes))
		} else if read == 0 {
			Ok(None)
		} else {
			Err(check_eof().await)
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
	pub async fn read_le<T: BytesEncoding<N>, const N: usize>(&mut self) -> Result<Option<T>> {
		/* for some llvm reason, unlikely here does the actual job of
		 * likely, and nets a performance gain on x64
		 */
		Ok(if unlikely(self.reader.buffer().len() >= N) {
			Some(T::from_bytes_le(self.read_bytes()))
		} else {
			self.read_bytes_slow().await?.map(T::from_bytes_le)
		})
	}

	#[inline(always)]
	pub async fn read_be<T: BytesEncoding<N>, const N: usize>(&mut self) -> Result<Option<T>> {
		Ok(if unlikely(self.reader.buffer().len() >= N) {
			Some(T::from_bytes_be(self.read_bytes()))
		} else {
			self.read_bytes_slow().await?.map(T::from_bytes_be)
		})
	}

	#[inline(always)]
	pub async fn read_le_or_eof<T: BytesEncoding<N>, const N: usize>(&mut self) -> Result<T> {
		self.read_le().await?.ok_or(eof_error())
	}

	#[inline(always)]
	pub async fn read_be_or_eof<T: BytesEncoding<N>, const N: usize>(&mut self) -> Result<T> {
		self.read_be().await?.ok_or(eof_error())
	}
}

impl<Context: AsyncContext, R: BufRead<Context>> Deref for TypedReader<Context, R> {
	type Target = R;

	fn deref(&self) -> &R {
		&self.reader
	}
}

impl<Context: AsyncContext, R: BufRead<Context>> DerefMut for TypedReader<Context, R> {
	fn deref_mut(&mut self) -> &mut R {
		&mut self.reader
	}
}

pub struct TypedWriter<Context: AsyncContext, W: Write<Context>> {
	writer: W,
	phantom: PhantomData<Context>
}

struct FmtAdapter<'a, Context: AsyncContext, W: 'a> {
	writer: &'a mut W,
	context: Handle<Context>,
	wrote: usize,
	error: Option<Error>
}

#[async_fn]
impl<'a, W: Write<Context>, Context: AsyncContext>
	FmtAdapter<'a, Context, TypedWriter<Context, W>>
{
	pub async fn new(
		writer: &'a mut TypedWriter<Context, W>
	) -> FmtAdapter<'a, Context, TypedWriter<Context, W>> {
		Self {
			writer,
			context: get_context().await,
			wrote: 0,
			error: None
		}
	}

	pub async fn write_args(&mut self, args: Arguments<'_>) -> Result<usize> {
		match fmt::write(self, args) {
			Ok(()) => Ok(self.wrote),
			Err(_) => Err(self
				.error
				.take()
				.unwrap_or(Error::new(ErrorKind::Other, "Formatter error")))
		}
	}
}

impl<W: Write<Context>, Context: AsyncContext> fmt::Write
	for FmtAdapter<'_, Context, TypedWriter<Context, W>>
{
	fn write_str(self: &mut Self, s: &str) -> fmt::Result {
		match self.context.run(self.writer.write_string_exact_or_err(s)) {
			Err(err) => {
				self.error = Some(err);

				Err(fmt::Error)
			}

			Ok(n) => {
				self.wrote += n;

				Ok(())
			}
		}
	}
}

#[async_fn]
impl<Context: AsyncContext, W: Write<Context>> TypedWriter<Context, W> {
	pub fn new(writer: W) -> Self {
		Self { writer, phantom: PhantomData }
	}

	pub fn into_inner(self) -> W {
		self.writer
	}

	/// Returns the number of bytes written, or error if the data could not be
	/// fully written
	pub async fn write_fmt(&mut self, args: Arguments<'_>) -> Result<usize> {
		FmtAdapter::new(self).await.write_args(args).await
	}

	pub async fn write_string(&mut self, buf: &str) -> Result<usize> {
		self.writer.write_all(buf.as_bytes()).await
	}

	pub async fn write_string_exact_or_err(&mut self, buf: &str) -> Result<usize> {
		self.writer.write_all_or_err(buf.as_bytes()).await?;

		Ok(buf.as_bytes().len())
	}

	pub async fn write_char(&mut self, ch: char) -> Result<usize> {
		let mut buf = [0u8; 4];

		self.write_string(ch.encode_utf8(&mut buf)).await
	}

	/// Write the number `val`, as little endian bytes
	#[inline(always)]
	pub async fn write_le<T: BytesEncoding<N>, const N: usize>(&mut self, val: T) -> Result<usize> {
		self.writer.write_all(&val.to_bytes_le()).await
	}

	/// Write the number `val`, as big endian bytes
	#[inline(always)]
	pub async fn write_be<T: BytesEncoding<N>, const N: usize>(&mut self, val: T) -> Result<usize> {
		self.writer.write_all(&val.to_bytes_be()).await
	}
}

impl<Context: AsyncContext, W: Write<Context>> Deref for TypedWriter<Context, W> {
	type Target = W;

	fn deref(&self) -> &W {
		&self.writer
	}
}

impl<Context: AsyncContext, W: Write<Context>> DerefMut for TypedWriter<Context, W> {
	fn deref_mut(&mut self) -> &mut W {
		&mut self.writer
	}
}
