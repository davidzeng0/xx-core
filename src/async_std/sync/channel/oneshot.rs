#![allow(clippy::needless_pass_by_ref_mut)]

pub use super::error::RecvError;
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
		self.sent
			.swap(false, Ordering::SeqCst)
			/* Safety: we took ownership of the value */
			.then(|| unsafe { ptr!(self.value=>assume_init_read()) })
	}

	fn close(&self) {
		self.tx_waiter.close(());
		self.rx_waiter.close(());
	}
}

/* Safety: only if T is send */
unsafe impl<T: Send> Send for Channel<T> {}

/* Safety: only if T is send */
unsafe impl<T: Send> Sync for Channel<T> {}

pub struct Receiver<T> {
	channel: Arc<Channel<T>>
}

#[asynchronous]
impl<T> Receiver<T> {
	pub fn try_recv(&mut self) -> RecvResult<T> {
		match self.channel.try_consume_value() {
			Some(value) => {
				self.channel.tx_waiter.close(());

				Ok(value)
			}

			None => Err(RecvError::new(self.channel.tx_waiter.is_closed()))
		}
	}

	pub async fn recv(&mut self) -> RecvResult<T> {
		let _ = self.channel.rx_waiter.wait_thread_safe().await;

		self.try_recv()
	}

	pub fn close(&mut self) {
		self.channel.close();
	}
}

#[asynchronous(task)]
impl<T> Task for Receiver<T> {
	type Output<'ctx> = RecvResult<T>;

	async fn run(mut self) -> RecvResult<T> {
		self.recv().await
	}
}

impl<T> Drop for Receiver<T> {
	fn drop(&mut self) {
		self.channel.tx_waiter.close(());

		/* the value may already be sent. try a read here to prevent leaking */
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

		self.channel.sent.store(true, Ordering::SeqCst);
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
		self.channel.tx_waiter.is_closed()
	}

	pub async fn closed(&mut self) -> bool {
		let _ = self.channel.tx_waiter.wait_thread_safe().await;

		self.is_closed()
	}
}

impl<T> Drop for Sender<T> {
	fn drop(&mut self) {
		self.channel.close();
	}
}

#[must_use]
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
	let channel = Arc::new(Channel::new());

	(Sender { channel: channel.clone() }, Receiver { channel })
}
