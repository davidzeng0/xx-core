use std::{
	fmt::{self, Arguments},
	mem::size_of
};

use super::*;
use crate::pointer::*;

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
	($type: ty, $endian: ident, $trait_endian: ident) => {
		paste::paste! {
			#[allow(non_camel_case_types)]
			struct [<$type $endian>](pub $type);

			impl ToBytes<{ size_of::<$type>() }> for [<$type $endian>] {
				fn to_bytes(self) -> [u8; size_of::<$type>()] {
					self.0.[<to_ $endian _bytes>]()
				}
			}

			impl FromBytes<{ size_of::<$type>() }> for [<$type $endian>] {
				fn from_bytes(bytes: [u8; size_of::<$type>()]) -> Self {
					Self($type::[<from_ $endian _bytes>](bytes))
				}
			}

			#[allow(dead_code)]
			impl [<$type $endian>] {
				pub const BYTES: usize = size_of::<$type>();
			}
		}
	};
}

macro_rules! impl_primitive_type {
	($type: ty, $bits: literal) => {
		paste::paste! {
			impl_primitive_bytes_encoding_endian!($type, le, LittleEndian);
			impl_primitive_bytes_encoding_endian!($type, be, BigEndian);
		}
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

macro_rules! read_num_type_endian {
	($type: ty, $endian_type: ty, $endian: ident) => {
		paste::paste! {
			#[asynchronous(explicit)]
			async fn [<read_ $endian_type>](&mut self) -> Result<Option<$type>> {
				self.read_type::<[<$type $endian>], { [<$type $endian>]::BYTES }>().await.map(|c| c.map(|t| t.0))
			}

			#[asynchronous(explicit)]
			async fn [<read_ $endian_type _or_err>](&mut self) -> Result<$type> {
				self.[<read_ $endian_type>]().await?.ok_or_else(|| Core::UnexpectedEof.into())
			}
		}
	};
}

macro_rules! read_num_type {
	($type: ty) => {
		paste::paste! {
			read_num_type_endian!($type, [<$type _le>], le);
			read_num_type_endian!($type, [<$type _be>], be);
		}
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

#[asynchronous]
async fn read_bytes_n<R: Read + ?Sized, const N: usize>(
	reader: &mut R, bytes: &mut [u8; N]
) -> Result<usize> {
	let read = reader.read_fully(bytes).await?;

	length_check(bytes, read);

	if unlikely(read != N) {
		check_interrupt().await?;

		if read != 0 {
			return Err(short_io_error_unless_interrupt().await);
		}
	}

	Ok(read)
}

#[asynchronous]
async fn read_bytes<R: Read + ?Sized, const N: usize>(reader: &mut R) -> Result<Option<[u8; N]>> {
	let mut bytes = [0u8; N];

	Ok(if read_bytes_n(reader, &mut bytes).await? != 0 {
		Some(bytes)
	} else {
		None
	})
}

#[asynchronous]
#[inline(never)]
async fn read_bytes_slow<R: Read + ?Sized, const N: usize>(
	reader: &mut R, bytes: &mut [u8; N]
) -> Result<usize> {
	read_bytes_n(reader, bytes).await
}

#[asynchronous]
async fn buf_read_bytes<R: BufRead + ?Sized, const N: usize>(
	reader: &mut R
) -> Result<Option<[u8; N]>> {
	/* for some llvm or rustc reason, unlikely here does the actual job of
	 * likely, and nets a performance gain on x64
	 */
	let available = reader.buffer();

	Ok(if unlikely(available.len() >= N) {
		/* bytes variable is separated to improve optimizations */
		let mut bytes = [0u8; N];

		/* this gets optimized to a single load instruction of size N */
		read_into_slice(&mut bytes, &available[0..N]);

		reader.consume(N);

		Some(bytes)
	} else {
		let mut bytes = [0u8; N];

		if read_bytes_slow(reader, &mut bytes).await? != 0 {
			Some(bytes)
		} else {
			None
		}
	})
}

pub trait ReadTyped: Read {
	read_num_type_endian!(i8, i8, le);

	read_num_type_endian!(u8, u8, le);

	read_int!(16);

	read_int!(32);

	read_int!(64);

	read_int!(128);

	read_num_type!(f32);

	read_num_type!(f64);

	/// Read a type, returning None if EOF and no bytes were read
	#[asynchronous(explicit)]
	async fn read_type<T: FromBytes<N>, const N: usize>(&mut self) -> Result<Option<T>> {
		let bytes = read_bytes(self).await?;

		Ok(bytes.map(T::from_bytes))
	}

	/// Read a type, assuming EOF is an error
	#[asynchronous(explicit)]
	async fn read_type_or_err<T: FromBytes<N>, const N: usize>(&mut self) -> Result<T> {
		self.read_type()
			.await?
			.ok_or_else(|| Core::UnexpectedEof.into())
	}
}

impl<T: Read> ReadTyped for T {}

pub trait BufReadTyped: BufRead {
	read_num_type_endian!(i8, i8, le);

	read_num_type_endian!(u8, u8, le);

	read_int!(16);

	read_int!(32);

	read_int!(64);

	read_int!(128);

	read_num_type!(f32);

	read_num_type!(f64);

	/// Read a type, returning None if EOF and no bytes were read
	#[asynchronous(explicit)]
	async fn read_type<T: FromBytes<N>, const N: usize>(&mut self) -> Result<Option<T>> {
		let bytes = buf_read_bytes(self).await?;

		Ok(bytes.map(T::from_bytes))
	}

	/// Read a type, assuming EOF is an error
	#[asynchronous(explicit)]
	async fn read_type_or_err<T: FromBytes<N>, const N: usize>(&mut self) -> Result<T> {
		self.read_type()
			.await?
			.ok_or_else(|| Core::UnexpectedEof.into())
	}
}

impl<T: BufRead> BufReadTyped for T {}

struct FmtAdapter<'a, W: Write + 'a> {
	writer: &'a mut W,
	context: Ptr<Context>,
	wrote: usize,
	error: Option<Error>
}

#[asynchronous]
impl<'a, W: Write> FmtAdapter<'a, W> {
	pub fn new(writer: &'a mut W, context: Ptr<Context>) -> FmtAdapter<'a, W> {
		Self { writer, context, wrote: 0, error: None }
	}

	pub async fn write_args(&mut self, args: Arguments<'_>) -> Result<usize> {
		match fmt::write(self, args) {
			Ok(()) => Ok(self.wrote),
			Err(_) => Err(self
				.error
				.take()
				.unwrap_or_else(|| Error::simple(ErrorKind::Other, "Formatter error")))
		}
	}
}

impl<W: Write> fmt::Write for FmtAdapter<'_, W> {
	fn write_str(&mut self, s: &str) -> fmt::Result {
		/* Safety: this is called from an async fn, so context is valid, and all our
		 * references */
		let result = unsafe { with_context(self.context, self.writer.write_string_all_or_err(s)) };

		match result {
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
	($type: ty, $endian_type: ident, $endian: ident) => {
		paste::paste! {
			#[asynchronous(explicit)]
			async fn [<write_ $endian_type>](&mut self, val: $type) -> Result<usize> {
				self.write_type::<[<$type $endian>], { [<$type $endian>]::BYTES }>([<$type $endian>](val)).await
			}
		}
	};
}

macro_rules! write_num_type {
	($type: ty) => {
		paste::paste! {
			write_num_type_endian!($type, [<$type _le>], le);
			write_num_type_endian!($type, [<$type _be>], be);
		}
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

pub trait WriteTyped: Write {
	write_num_type_endian!(i8, i8, le);

	write_num_type_endian!(u8, u8, le);

	write_int!(32);

	write_int!(16);

	write_int!(64);

	write_int!(128);

	write_num_type!(f32);

	write_num_type!(f64);

	/// Returns the number of bytes written, or error if the data could not be
	/// fully written
	#[asynchronous(explicit)]
	async fn write_fmt(&mut self, args: Arguments<'_>) -> Result<usize>
	where
		Self: Sized
	{
		FmtAdapter::new(self, get_context().await)
			.write_args(args)
			.await
	}

	/// Attempts to write the entire string, returning the number of bytes
	/// written which may be short if interrupted or eof
	#[asynchronous(explicit)]
	async fn write_string(&mut self, buf: &str) -> Result<usize> {
		self.write_all(buf.as_bytes()).await
	}

	/// Same as above but returns error on partial writes
	#[asynchronous(explicit)]
	async fn write_string_all_or_err(&mut self, buf: &str) -> Result<usize> {
		self.write_exact(buf.as_bytes()).await
	}

	/// Attemps to write an entire char, returning error on partial writes
	#[asynchronous(explicit)]
	async fn write_char(&mut self, ch: char) -> Result<usize> {
		let mut buf = [0u8; 4];

		self.write_string_all_or_err(ch.encode_utf8(&mut buf)).await
	}

	/// Attempts to write an entire type, returning the number of bytes written
	/// which may be short if interrupted or eof
	#[asynchronous(explicit)]
	async fn write_type<T: ToBytes<N>, const N: usize>(&mut self, val: T) -> Result<usize> {
		self.write_all(&val.to_bytes()).await
	}
}

impl<T: Write> WriteTyped for T {}
