#![allow(clippy::module_name_repetitions)]

use super::*;

#[asynchronous]
async fn default_write_vectored<W>(
	writer: &mut W, mut bufs: &mut [IoSlice<'_>]
) -> Result<(usize, bool)>
where
	W: Write + ?Sized
{
	let mut total = 0;

	while !bufs.is_empty() {
		let wrote = match writer.write_vectored(bufs).await {
			Ok(0) => break,
			Ok(n) => n,
			Err(err) if err.is_interrupted() => break,
			Err(err) => return Err(err)
		};

		advance_slices(&mut bufs, wrote);

		/* checked by `advance_slices` */
		#[allow(clippy::arithmetic_side_effects)]
		(total += wrote);
	}

	Ok((total, bufs.is_empty()))
}

/// The async equivalent of [`std::io::Write`]
///
/// This trait is object safe
#[asynchronous]
pub trait Write {
	/// Write from `buf`, returning the amount of bytes wrote
	///
	/// Returns zero if `buf` is empty, or if the stream reached EOF
	///
	/// See also [`std::io::Write::write`]
	async fn write(&mut self, buf: &[u8]) -> Result<usize>;

	/// Flush (if any) buffered data
	async fn flush(&mut self) -> Result<()> {
		Ok(())
	}

	/// Try to write the entire buffer, returning on I/O error, interrupt, or
	/// EOF
	///
	/// On interrupted, returns the number of bytes written if it is not zero
	///
	/// See also [`std::io::Write::try_write_all`]
	async fn try_write_all(&mut self, buf: &[u8]) -> Result<usize> {
		write_from!(buf);

		let mut wrote = 0;

		while wrote < buf.len() {
			let available = &buf[wrote..];

			#[allow(clippy::arithmetic_side_effects)]
			match self.write(available).await {
				Ok(0) => break,
				Ok(n) => wrote += length_check(available, n),
				Err(err) if err.is_interrupted() => break,
				Err(err) => return Err(err)
			}
		}

		check_interrupt_if_zero(wrote).await
	}

	/// Same as [`Write::try_write_all`], except it returns an [`UnexpectedEof`]
	/// on partial writes
	///
	/// [`UnexpectedEof`]: ErrorKind::UnexpectedEof
	async fn write_all(&mut self, buf: &[u8]) -> Result<usize> {
		write_from!(buf);

		let wrote = self.try_write_all(buf).await?;

		length_check(buf, wrote);

		if wrote < buf.len() {
			return Err(short_io_error_unless_interrupt().await);
		}

		Ok(wrote)
	}

	/// Returns `true` if this `Write` implementation has an efficient
	/// [`Write::write_vectored`] implementation
	///
	/// See also [`std::io::Write::is_write_vectored`]
	fn is_write_vectored(&self) -> bool {
		false
	}

	/// Like [`Write::write`], except that it writes from a slice of buffers
	///
	/// See also [`std::io::Write::write_vectored`]
	async fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
		match bufs.iter().find(|b| !b.is_empty()) {
			Some(buf) => self.write(&buf[..]).await,
			None => Ok(0)
		}
	}

	/// Like [`write_vectored`], except that it keeps writing until all the
	/// buffers are exhausted
	///
	/// Returns on EOF
	///
	/// [`write_vectored`]: Write::write_vectored
	async fn try_write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> Result<usize> {
		Ok(default_write_vectored(self, bufs).await?.0)
	}

	/// Same as [`try_write_all_vectored`], except returns an [`UnexpectedEof`]
	/// error on partial writes
	///
	/// Returns the number of bytes written, which is the same as
	/// the length of all the buffers
	///
	/// [`try_write_all_vectored`]: Write::try_write_all_vectored
	/// [`UnexpectedEof`]: ErrorKind::UnexpectedEof
	async fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> Result<usize> {
		let (wrote, exhausted) = default_write_vectored(self, bufs).await?;

		if unlikely(!exhausted) {
			return Err(short_io_error_unless_interrupt().await);
		}

		Ok(wrote)
	}
}

#[macro_export]
macro_rules! write_wrapper {
	{
		inner = $inner: expr;
		mut inner = $inner_mut: expr;
	} => {
		$crate::macros::wrapper_functions! {
			inner = self.$inner;
			mut inner = self.$inner_mut;

			#[asynchronous(traitfn)]
			async fn write(&mut self, buf: &[u8]) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn flush(&mut self) -> $crate::error::Result<()>;

			#[asynchronous(traitfn)]
			async fn try_write_all(&mut self, buf: &[u8]) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn write_all(&mut self, buf: &[u8]) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			fn is_write_vectored(&self) -> bool;

			#[asynchronous(traitfn)]
			async fn write_vectored(&mut self, bufs: &[::std::io::IoSlice<'_>]) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn try_write_all_vectored(&mut self, bufs: &mut [::std::io::IoSlice<'_>]) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn write_all_vectored(&mut self, bufs: &mut [::std::io::IoSlice<'_>]) -> $crate::error::Result<usize>;
		}
	}
}

pub use write_wrapper;
