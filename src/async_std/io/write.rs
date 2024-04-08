#![allow(clippy::module_name_repetitions)]

use super::*;

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
	/// On interrupted, returns the number of bytes read if it is not zero
	///
	/// See also [`std::io::Write::write_all`]
	async fn write_all(&mut self, buf: &[u8]) -> Result<usize> {
		/* see Read::read_exact */
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

	/// Same as [`Write::write_all`], except it returns an [`UnexpectedEof`] on
	/// partial writes
	///
	/// [`UnexpectedEof`]: Core::UnexpectedEof
	async fn write_exact(&mut self, buf: &[u8]) -> Result<usize> {
		write_from!(buf);

		let wrote = self.write_all(buf).await?;

		length_check(buf, wrote);

		if unlikely(wrote != buf.len()) {
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
	/// See also [`std::io::Read::read_vectored`]
	async fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
		match bufs.iter().find(|b| !b.is_empty()) {
			Some(buf) => self.write(&buf[..]).await,
			None => Ok(0)
		}
	}

	async fn write_all_vectored(&mut self, mut bufs: &mut [IoSlice<'_>]) -> Result<usize> {
		let mut total = 0;

		while !bufs.is_empty() {
			let wrote = match self.write_vectored(bufs).await {
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

		Ok(total)
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
			async fn write_all(&mut self, buf: &[u8]) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn write_exact(&mut self, buf: &[u8]) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			fn is_write_vectored(&self) -> bool;

			#[asynchronous(traitfn)]
			async fn write_vectored(&mut self, bufs: &[::std::io::IoSlice<'_>]) -> $crate::error::Result<usize>;
		}
	}
}

pub use write_wrapper;
