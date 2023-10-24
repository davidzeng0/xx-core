use std::{
	fmt::{self, Arguments},
	marker::PhantomData,
	ops::{Deref, DerefMut}
};

use super::{BufRead, ReadExt, Write, WriteExt};
use crate::{coroutines::*, error::*, opt::hint::*, task::Handle, xx_core};

pub trait ToBytes<const N: usize> {
	fn to_bytes(self) -> [u8; N];
}

pub trait FromBytes<const N: usize> {
	fn from_bytes(bytes: [u8; N]) -> Self;
}

impl<const N: usize> ToBytes<N> for [u8; N] {
	fn to_bytes(self) -> [u8; N] {
		self
	}
}

impl<const N: usize> FromBytes<N> for [u8; N] {
	fn from_bytes(bytes: [u8; N]) -> Self {
		bytes
	}
}

macro_rules! impl_primitive_bytes_encoding_endian {
	($type: ty, $bytes: expr, $endian: ident, $trait_endian: ident) => {
		paste::paste! {
			#[allow(non_camel_case_types)]
			struct [<$type $endian>](pub $type);

			impl ToBytes<{ $bytes }> for [<$type $endian>] {
				#[inline(always)]
				fn to_bytes(self) -> [u8; $bytes] {
					self.0.[<to_ $endian _bytes>]()
				}
			}

			impl FromBytes<{ $bytes }> for [<$type $endian>] {
				#[inline(always)]
				fn from_bytes(bytes: [u8; $bytes]) -> Self {
					Self($type::[<from_ $endian _bytes>](bytes))
				}
			}

			impl [<$type $endian>] {
				pub const BYTES: usize = $bytes;
			}
		}
	};
}

macro_rules! impl_primitive_type {
	($type: ty, $bits: literal) => {
		impl_primitive_bytes_encoding_endian!($type, $bits / 8 as usize, le, LittleEndian);
		impl_primitive_bytes_encoding_endian!($type, $bits / 8 as usize, be, BigEndian);
	};
}

macro_rules! impl_int {
	($bits: literal) => {
		paste::paste! {
			impl_primitive_type!([<i $bits>], $bits);
			impl_primitive_type!([<u $bits>], $bits);
		}
	};
}

/* usize and isize omitted intentionally */
impl_int!(8);
impl_int!(16);
impl_int!(32);
impl_int!(64);
impl_int!(128);
impl_primitive_type!(f32, 32);
impl_primitive_type!(f64, 64);

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

macro_rules! read_num_type_endian {
	($type: ty, $endian: ident) => {
		paste::paste! {
			#[inline(always)]
			#[async_fn]
			pub async fn [<read_ $type _ $endian>](&mut self) -> Result<Option<$type>> {
				self.read_type::<[<$type $endian>], { [<$type $endian>]::BYTES }>().await.map(|c| c.map(|t| t.0))
			}

			#[inline(always)]
			#[async_fn]
			pub async fn [<read_ $type _ $endian _or_err>](&mut self) -> Result<$type> {
				self.[<read_ $type _ $endian>]().await?.ok_or(eof_error())
			}
		}
	}
}

macro_rules! read_num_type {
	($type: ty) => {
		read_num_type_endian!($type, le);
		read_num_type_endian!($type, be);
	};
}

macro_rules! read_int {
	($bits: literal) => {
		paste::paste! {
			read_num_type!([<i $bits>]);
			read_num_type!([<u $bits>]);
		}
	};
}

#[async_fn]
impl<Context: AsyncContext, R: BufRead<Context>> TypedReader<Context, R> {
	read_int!(8);

	read_int!(16);

	read_int!(32);

	read_int!(64);

	read_int!(128);

	read_num_type!(f32);

	read_num_type!(f64);

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
	pub async fn read_type<T: FromBytes<N>, const N: usize>(&mut self) -> Result<Option<T>> {
		/* for some llvm reason, unlikely here does the actual job of
		 * likely, and nets a performance gain on x64
		 */
		Ok(if unlikely(self.reader.buffer().len() >= N) {
			Some(T::from_bytes(self.read_bytes()))
		} else {
			self.read_bytes_slow().await?.map(T::from_bytes)
		})
	}

	#[inline(always)]
	pub async fn read_type_or_err<T: FromBytes<N>, const N: usize>(&mut self) -> Result<T> {
		self.read_type().await?.ok_or(eof_error())
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

macro_rules! write_num_type_endian {
	($type: ty, $endian: ident) => {
		paste::paste! {
			#[inline(always)]
			#[async_fn]
			pub async fn [<write_ $type _ $endian>](&mut self, val: $type) -> Result<usize> {
				self.write_type::<[<$type $endian>], { [<$type $endian>]::BYTES }>([<$type $endian>](val)).await
			}
		}
	}
}

macro_rules! write_num_type {
	($type: ty) => {
		write_num_type_endian!($type, le);
		write_num_type_endian!($type, be);
	};
}

macro_rules! write_int {
	($bits: literal) => {
		paste::paste! {
			write_num_type!([<i $bits>]);
			write_num_type!([<u $bits>]);
		}
	};
}

#[async_fn]
impl<Context: AsyncContext, W: Write<Context>> TypedWriter<Context, W> {
	write_int!(8);

	write_int!(32);

	write_int!(16);

	write_int!(64);

	write_int!(128);

	write_num_type!(f32);

	write_num_type!(f64);

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

	#[inline(always)]
	pub async fn write_type<T: ToBytes<N>, const N: usize>(&mut self, val: T) -> Result<usize> {
		self.writer.write_all(&val.to_bytes()).await
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
