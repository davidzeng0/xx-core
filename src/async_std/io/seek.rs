use super::*;

#[asynchronous]
pub trait Seek {
	async fn seek(&mut self, seek: SeekFrom) -> Result<u64>;

	/// Whether or not stream length can be calculated without an
	/// expensive I/O operation
	fn stream_len_fast(&self) -> bool {
		false
	}

	/// Get the length of the stream in bytes
	async fn stream_len(&mut self) -> Result<u64> {
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
	async fn stream_position(&mut self) -> Result<u64> {
		self.seek(SeekFrom::Current(0)).await
	}

	/// Rewinds the stream to the beginning
	async fn rewind(&mut self) -> Result<()> {
		self.seek(SeekFrom::Start(0)).await?;

		Ok(())
	}

	/// Rewind `amount` bytes on the stream
	async fn rewind_exact(&mut self, amount: u64) -> Result<u64> {
		let amount: i64 = amount.try_into().map_err(|_| Core::Overflow.as_err())?;

		self.seek(SeekFrom::Current(-amount)).await
	}

	/// Skips `amount` bytes from the stream
	async fn skip_exact(&mut self, amount: u64) -> Result<u64> {
		self.seek(SeekFrom::Current(
			amount.try_into().map_err(|_| Core::Overflow.as_err())?
		))
		.await
	}
}

#[macro_export]
macro_rules! seek_wrapper {
	{
		inner = $inner: expr;
		mut inner = $inner_mut: expr;
	} => {
		$crate::macros::wrapper_functions! {
			inner = self.$inner;
			mut inner = self.$inner_mut;

			#[asynchronous(traitfn)]
			async fn seek(&mut self, seek: std::io::SeekFrom) -> Result<u64>;

			#[asynchronous(traitfn)]
			fn stream_len_fast(&self) -> bool;

			#[asynchronous(traitfn)]
			async fn stream_len(&mut self) -> Result<u64>;

			#[asynchronous(traitfn)]
			fn stream_position_fast(&self) -> bool;

			#[asynchronous(traitfn)]
			async fn stream_position(&mut self) -> Result<u64>;

			#[asynchronous(traitfn)]
			async fn rewind(&mut self) -> Result<()>;

			#[asynchronous(traitfn)]
			async fn rewind_exact(&mut self, amount: u64) -> Result<u64>;

			#[asynchronous(traitfn)]
			async fn skip_exact(&mut self, amount: u64) -> Result<u64>;
		}
	}
}

pub use seek_wrapper;
