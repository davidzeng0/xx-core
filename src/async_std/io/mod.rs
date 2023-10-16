use std::{
	io::{Result, SeekFrom},
	marker::PhantomData,
	ops::{Deref, DerefMut}
};

use crate::{
	coroutines::{async_fn, async_trait_fn, env::AsyncContext, runtime::get_context},
	xx_core
};

pub mod buf;

pub use buf::*;

use super::{AsyncIterator, Iterator};

#[async_trait_fn]
pub trait Read<Context: AsyncContext> {
	async fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
}

#[async_trait_fn]
pub trait Write<Context: AsyncContext> {
	async fn write(&mut self, buf: &[u8]) -> Result<usize>;

	async fn flush(&mut self) -> Result<()>;
}

#[async_trait_fn]
pub trait Seek<Context: AsyncContext> {
	async fn seek(&mut self, seek: SeekFrom) -> Result<u64>;

	/// Whether or not stream length can be calculated without doing anything
	/// expensive
	fn stream_len_fast(&self) -> bool {
		false
	}

	async fn stream_len(&mut self) -> Result<u64> {
		let old_pos = self.stream_position(get_context().await)?;
		let len = self.seek(SeekFrom::End(0), get_context().await)?;

		if old_pos != len {
			self.seek(SeekFrom::Start(old_pos), get_context().await)?;
		}

		Ok(len)
	}

	/// Whether or not stream position can be calculated without doing anything
	/// expensive
	fn stream_position_fast(&self) -> bool {
		false
	}

	async fn stream_position(&mut self) -> Result<u64> {
		self.seek(SeekFrom::Current(0), get_context().await)
	}
}

#[async_trait_fn]
pub trait Close<Context: AsyncContext> {
	async fn close(self) -> Result<()>;
}

pub struct Stream<Context: AsyncContext, Inner> {
	inner: Inner,
	phantom: PhantomData<Context>
}

impl<Context: AsyncContext, Inner> Stream<Context, Inner> {
	pub fn new(inner: Inner) -> Self {
		Self { inner, phantom: PhantomData }
	}

	pub fn into_inner(self) -> Inner {
		self.inner
	}
}

#[async_fn]
impl<Context: AsyncContext, Inner: Read<Context>> Stream<Context, Inner> {
	pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
		self.inner.read(buf, get_context().await)
	}

	pub async fn read_exact(&mut self, buf: &mut [u8]) -> Result<usize> {
		let mut offset = 0;

		while offset < buf.len() {
			let read = self.read(&mut buf[offset..]).await?;

			if read == 0 {
				break;
			}

			offset += read;
		}

		Ok(offset)
	}

	pub async fn read_fully(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
		let start_len = buf.len();

		loop {
			let mut capacity = buf.capacity();
			let len = buf.len();

			if len == capacity {
				buf.reserve(32);
			}

			unsafe {
				let read;

				capacity = buf.capacity();
				read = self.read(buf.get_unchecked_mut(len..capacity)).await?;

				if read == 0 {
					break;
				}

				buf.set_len(len + read);
			}

			if buf.len() == capacity {
				let mut probe = [0u8; 32];
				let read = self.read(&mut probe).await?;

				if read == 0 {
					break;
				}

				buf.extend_from_slice(&probe[0..read]);
			}
		}

		Ok(buf.len() - start_len)
	}
}

#[async_trait_fn]
impl<Context: AsyncContext, Inner: Read<Context>> Read<Context> for Stream<Context, Inner> {
	async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
		self.read(buf).await
	}
}

#[async_fn]
impl<Context: AsyncContext, Inner: Write<Context>> Stream<Context, Inner> {
	pub async fn write(&mut self, buf: &[u8]) -> Result<usize> {
		self.inner.write(buf, get_context().await)
	}

	pub async fn write_exact(&mut self, buf: &[u8]) -> Result<usize> {
		let mut offset = 0;

		while offset < buf.len() {
			let read = self.write(&buf[offset..]).await?;

			if read == 0 {
				break;
			}

			offset += read;
		}

		Ok(offset)
	}

	pub async fn flush(&mut self) -> Result<()> {
		self.inner.flush(get_context().await)
	}
}

#[async_trait_fn]
impl<Context: AsyncContext, Inner: Write<Context>> Write<Context> for Stream<Context, Inner> {
	async fn write(&mut self, buf: &[u8]) -> Result<usize> {
		self.write(buf).await
	}

	async fn flush(&mut self) -> Result<()> {
		self.flush().await
	}
}

#[async_fn]
impl<Context: AsyncContext, Inner: Seek<Context>> Stream<Context, Inner> {
	pub async fn seek(&mut self, seek: SeekFrom) -> Result<u64> {
		self.inner.seek(seek, get_context().await)
	}

	pub async fn rewind(&mut self) -> Result<()> {
		self.seek(SeekFrom::Start(0)).await?;

		Ok(())
	}

	pub async fn rewind_exact(&mut self, amount: i64) -> Result<u64> {
		self.seek(SeekFrom::Current(-amount)).await
	}

	pub async fn skip_exact(&mut self, amount: i64) -> Result<u64> {
		self.seek(SeekFrom::Current(amount)).await
	}

	pub fn stream_len_fast(&self) -> bool {
		self.inner.stream_len_fast()
	}

	pub async fn stream_len(&mut self) -> Result<u64> {
		self.inner.stream_len(get_context().await)
	}

	pub fn stream_position_fast(&self) -> bool {
		self.inner.stream_position_fast()
	}

	pub async fn stream_position(&mut self) -> Result<u64> {
		self.inner.stream_position(get_context().await)
	}
}

#[async_trait_fn]
impl<Context: AsyncContext, Inner: Seek<Context>> Seek<Context> for Stream<Context, Inner> {
	async fn seek(&mut self, seek: SeekFrom) -> Result<u64> {
		self.seek(seek).await
	}

	async fn stream_len(&mut self) -> Result<u64> {
		self.stream_len().await
	}

	async fn stream_position(&mut self) -> Result<u64> {
		self.stream_position().await
	}
}

#[async_fn]
impl<Context: AsyncContext, Inner: Close<Context>> Stream<Context, Inner> {
	pub async fn close(self) -> Result<()> {
		self.inner.close(get_context().await)
	}
}

#[async_trait_fn]
impl<Context: AsyncContext, Inner: Close<Context>> Close<Context> for Stream<Context, Inner> {
	async fn close(self) -> Result<()> {
		self.close().await
	}
}

impl<Context: AsyncContext, Inner> Deref for Stream<Context, Inner> {
	type Target = Inner;

	fn deref(&self) -> &Inner {
		&self.inner
	}
}

impl<Context: AsyncContext, Inner> DerefMut for Stream<Context, Inner> {
	fn deref_mut(&mut self) -> &mut Inner {
		&mut self.inner
	}
}

pub struct Lines<Context: AsyncContext, R: Read<Context>> {
	reader: BufReader<Context, R>
}

impl<Context: AsyncContext, R: Read<Context>> Lines<Context, R> {
	pub fn new(reader: BufReader<Context, R>) -> Iterator<Context, Self> {
		Iterator::new(Self { reader })
	}
}

#[async_trait_fn]
impl<Context: AsyncContext, R: Read<Context>> AsyncIterator<Context> for Lines<Context, R> {
	type Item = Result<String>;

	async fn next(&mut self) -> Option<Self::Item> {
		let mut line = String::new();

		match self.reader.read_line(&mut line).await {
			Err(err) => Some(Err(err)),
			Ok(Some(_)) => Some(Ok(line)),
			Ok(None) => None
		}
	}
}
