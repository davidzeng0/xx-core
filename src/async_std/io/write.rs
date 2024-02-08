use super::*;

#[asynchronous]
pub trait Write {
	/// Write from `buf`, returning the amount of bytes read
	///
	/// Returning zero strictly means EOF, unless the buffer's size was zero
	async fn write(&mut self, buf: &[u8]) -> Result<usize>;

	/// Flush (if any) buffered data
	async fn flush(&mut self) -> Result<()> {
		Ok(())
	}

	/// Try to write the entire buffer, returning on I/O error, interrupt, or
	/// EOF
	///
	/// On interrupted, returns the number of bytes read if it is not zero
	async fn write_all(&mut self, buf: &[u8]) -> Result<usize> {
		/* see Read::read_exact */
		write_from!(buf);

		let mut wrote = 0;

		while wrote < buf.len() {
			let available = &buf[wrote..];

			match self.write(available).await {
				Ok(0) => break,
				Ok(n) => wrote += length_check(available, n),
				Err(err) if err.is_interrupted() => break,
				Err(err) => return Err(err)
			}
		}

		check_interrupt_if_zero(wrote).await
	}

	/// Same as above, except partial writes are treated as an error
	async fn write_exact(&mut self, buf: &[u8]) -> Result<usize> {
		let wrote = self.write_all(buf).await?;

		length_check(buf, wrote);

		if unlikely(wrote != buf.len()) {
			return Err(short_io_error_unless_interrupt().await);
		}

		Ok(wrote)
	}

	fn is_write_vectored(&self) -> bool {
		false
	}

	async fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
		match bufs.iter().find(|b| !b.is_empty()) {
			Some(buf) => self.write(&buf[..]).await,
			None => Ok(0)
		}
	}

	async fn write_all_vectored(&mut self, mut bufs: &mut [IoSlice<'_>]) -> Result<usize> {
		let mut total = 0;

		while bufs.len() > 0 {
			let wrote = match self.write_vectored(bufs).await {
				Ok(0) => break,
				Ok(n) => n,
				Err(err) if err.is_interrupted() => break,
				Err(err) => return Err(err)
			};

			total += wrote;

			advance_slices(&mut bufs, wrote);
		}

		Ok(total)
	}
}

pub trait AsWriteRef: Write {
	fn as_ref(&mut self) -> WriteRef<'_, Self> {
		WriteRef::new(self)
	}
}

impl<T: Write> AsWriteRef for T {}

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
			async fn write(&mut self, buf: &[u8]) -> Result<usize>;

			#[asynchronous(traitfn)]
			async fn flush(&mut self) -> Result<()>;

			#[asynchronous(traitfn)]
			async fn write_all(&mut self, buf: &[u8]) -> Result<usize>;

			#[asynchronous(traitfn)]
			async fn write_exact(&mut self, buf: &[u8]) -> Result<usize>;

			#[asynchronous(traitfn)]
			fn is_write_vectored(&self) -> bool;

			#[asynchronous(traitfn)]
			async fn write_vectored(&mut self, bufs: &[std::io::IoSlice<'_>]) -> Result<usize>;
		}
	}
}

pub struct WriteRef<'a, W: Write + ?Sized> {
	writer: &'a mut W
}

impl<'a, W: Write + ?Sized> WriteRef<'a, W> {
	pub fn new(writer: &'a mut W) -> Self {
		Self { writer }
	}
}

impl<'a, W: Write + ?Sized> Write for WriteRef<'a, W> {
	write_wrapper! {
		inner = writer;
		mut inner = writer;
	}
}
