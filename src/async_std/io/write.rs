use std::{io::IoSlice, marker::PhantomData};

use crate::{async_std::ext::ext_func, coroutines::*, error::*, xx_core};

#[async_trait_fn]
pub trait Write<Context: AsyncContext> {
	/// Write from `buf`, returning the amount of bytes read
	///
	/// Returning zero strictly means EOF, unless the buffer's size was zero
	async fn async_write(&mut self, buf: &[u8]) -> Result<usize>;

	/// Flush buffered data
	async fn async_flush(&mut self) -> Result<()>;

	/// Try to write the entire buffer, returning on I/O error, interrupt, or
	/// EOF
	///
	/// On interrupted, returns the number of bytes read if it is not zero
	async fn async_write_all(&mut self, buf: &[u8]) -> Result<usize> {
		let mut wrote = 0;

		while wrote < buf.len() {
			match self.write(&buf[wrote..]).await {
				Ok(0) => break,
				Ok(n) => wrote += n,
				Err(err) => {
					if err.is_interrupted() {
						break;
					}

					return Err(err);
				}
			}
		}

		if wrote == 0 {
			check_interrupt().await?;
		}

		Ok(wrote)
	}

	fn is_write_vectored(&self) -> bool {
		false
	}

	async fn async_write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
		let buf = match bufs.iter().find(|b| !b.is_empty()) {
			Some(buf) => buf,
			None => return Ok(0)
		};

		self.write(&**buf).await
	}
}

pub struct WriteRef<'a, Context: AsyncContext, W: Write<Context>> {
	writer: &'a mut W,
	phantom: PhantomData<Context>
}

macro_rules! async_alias_func {
	($func: ident ($self: ident: $self_type: ty $(, $arg: ident: $type: ty)*) -> $return_type: ty) => {
		#[xx_core::coroutines::async_trait_fn]
		async fn $func($self: $self_type $(, $arg: $type)*) -> $return_type {
			$self.writer.$func($($arg,)* xx_core::coroutines::runtime::get_context().await)
		}
    }
}

macro_rules! alias_func {
	($func: ident ($self: ident: $self_type: ty $(, $arg: ident: $type: ty)*) -> $return_type: ty) => {
		fn $func($self: $self_type $(, $arg: $type)*) -> $return_type {
			$self.writer.$func($($arg,)*)
		}
    }
}

impl<Context: AsyncContext, W: Write<Context>> Write<Context> for WriteRef<'_, Context, W> {
	async_alias_func!(async_write(self: &mut Self, buf: &[u8]) -> Result<usize>);

	async_alias_func!(async_flush(self: &mut Self) -> Result<()>);

	async_alias_func!(async_write_all(self: &mut Self, buf: &[u8]) -> Result<usize>);

	async_alias_func!(async_write_vectored(self: &mut Self, bufs: &[IoSlice<'_>]) -> Result<usize>);

	alias_func!(is_write_vectored(self: &Self) -> bool);
}

pub trait WriteExt<Context: AsyncContext>: Write<Context> {
	ext_func!(write(self: &mut Self, buf: &[u8]) -> Result<usize>);

	ext_func!(flush(self: &mut Self) -> Result<()>);

	ext_func!(write_all(self: &mut Self, buf: &[u8]) -> Result<usize>);

	ext_func!(write_vectored(self: &mut Self, bufs: &[IoSlice<'_>]) -> Result<usize>);

	fn as_ref(&mut self) -> WriteRef<'_, Context, Self>
	where
		Self: Sized
	{
		WriteRef { writer: self, phantom: PhantomData }
	}
}

impl<Context: AsyncContext, T: ?Sized + Write<Context>> WriteExt<Context> for T {}