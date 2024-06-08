#![allow(clippy::needless_pass_by_ref_mut)]

use std::{
	mem::MaybeUninit,
	result,
	sync::{atomic::*, Arc}
};

use super::*;

struct Channel<T> {
	sent: AtomicBool,
	value: UnsafeCell<MaybeUninit<T>>,
	tx_waiter: AtomicWaiter,
	rx_waiter: AtomicWaiter
}

impl<T> Channel<T> {
	const fn new() -> Self {
		Self {
			sent: AtomicBool::new(false),
			value: UnsafeCell::new(MaybeUninit::uninit()),
			tx_waiter: AtomicWaiter::new(),
			rx_waiter: AtomicWaiter::new()
		}
	}

	#[allow(clippy::multiple_unsafe_ops_per_block)]
	fn try_consume_value(&self) -> Option<T> {
		match self.sent.swap(false, Ordering::Acquire) {
			/* Safety: we took ownership of the value */
			true => Some(unsafe { ptr!(self.value=>assume_init_read()) }),
			false => None
		}
	}
}

/* Safety: only if T is send */
unsafe impl<T: Send> Send for Channel<T> {}

/* Safety: only if T is send */
unsafe impl<T: Send> Sync for Channel<T> {}

#[errors]
pub enum RecvError {
	#[error("Channel empty")]
	#[kind = ErrorKind::WouldBlock]
	Empty,

	#[error("Channel closed")]
	Closed
}

pub struct Receiver<T> {
	channel: Arc<Channel<T>>
}

#[asynchronous]
impl<T> Receiver<T> {
	pub fn try_recv(&mut self) -> result::Result<T, RecvError> {
		match self.channel.try_consume_value() {
			Some(value) => {
				self.channel.tx_waiter.close(());

				Ok(value)
			}

			None => Err(if self.channel.tx_waiter.is_closed() {
				RecvError::Closed
			} else {
				RecvError::Empty
			})
		}
	}

	pub async fn recv(&mut self) -> result::Result<T, RecvError> {
		let _ = self.channel.rx_waiter.notified_thread_safe().await;

		self.try_recv()
	}

	pub fn close(&mut self) {
		self.channel.tx_waiter.close(());
	}
}

#[asynchronous(task)]
impl<T> Task for Receiver<T> {
	type Output<'ctx> = result::Result<T, RecvError>;

	async fn run(mut self) -> result::Result<T, RecvError> {
		self.recv().await
	}
}

impl<T> Drop for Receiver<T> {
	fn drop(&mut self) {
		self.close();

		let _ = self.channel.try_consume_value();
	}
}

pub struct Sender<T> {
	channel: Arc<Channel<T>>
}

#[asynchronous]
impl<T> Sender<T> {
	pub fn send(self, value: T) -> result::Result<(), T> {
		/* Safety: first time storing a value */
		unsafe { ptr!(self.channel.value=>write(value)) };

		self.channel.sent.store(true, Ordering::Release);
		self.channel.rx_waiter.close(());

		if !self.channel.tx_waiter.is_closed() {
			return Ok(());
		}

		/* the receiver closed, so they may not receive the value. try a read here to
		 * prevent leaking
		 */
		match self.channel.try_consume_value() {
			None => Ok(()),
			Some(value) => Err(value)
		}
	}

	#[must_use]
	pub fn is_closed(&self) -> bool {
		self.channel.rx_waiter.is_closed()
	}

	pub async fn closed(&mut self) -> bool {
		let _ = self.channel.tx_waiter.notified_thread_safe().await;

		self.is_closed()
	}
}

impl<T> Drop for Sender<T> {
	fn drop(&mut self) {
		self.channel.tx_waiter.close(());
		self.channel.rx_waiter.close(());
	}
}

#[must_use]
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
	let channel = Arc::new(Channel::new());

	(Sender { channel: channel.clone() }, Receiver { channel })
}
