use std::io::SeekFrom;

use crate::{async_std::ext::ext_func, coroutines::*, error::Result, xx_core};

#[async_trait_fn]
pub trait Seek<Context: AsyncContext> {
	async fn async_seek(&mut self, seek: SeekFrom) -> Result<u64>;

	/// Whether or not stream length can be calculated without an
	/// expensive I/O operation
	fn stream_len_fast(&self) -> bool {
		false
	}

	/// Get the length of the stream in bytes
	async fn async_stream_len(&mut self) -> Result<u64> {
		let old_pos = self.stream_position().await?;
		let len = self.seek(SeekFrom::End(0)).await?;

		if old_pos != len {
			self.seek(SeekFrom::Start(old_pos)).await?;
		}

		Ok(len)
	}

	/// Whether or not stream length can be calculated without an
	/// expensive I/O operation
	fn stream_position_fast(&self) -> bool {
		false
	}

	/// Get the position in the stream in bytes
	async fn async_stream_position(&mut self) -> Result<u64> {
		self.seek(SeekFrom::Current(0)).await
	}
}

pub trait SeekExt<Context: AsyncContext>: Seek<Context> {
	ext_func!(seek(self: &mut Self, seek: SeekFrom) -> Result<u64>);

	ext_func!(stream_len(self: &mut Self) -> Result<u64>);

	ext_func!(stream_position(self: &mut Self) -> Result<u64>);

	#[async_trait_impl]
	async fn rewind(&mut self) -> Result<()> {
		self.seek(SeekFrom::Start(0)).await?;

		Ok(())
	}

	#[async_trait_impl]
	async fn rewind_exact(&mut self, amount: i64) -> Result<u64> {
		self.seek(SeekFrom::Current(-amount)).await
	}

	#[async_trait_impl]
	async fn skip_exact(&mut self, amount: i64) -> Result<u64> {
		self.seek(SeekFrom::Current(amount)).await
	}
}

impl<Context: AsyncContext, T: ?Sized + Seek<Context>> SeekExt<Context> for T {}
