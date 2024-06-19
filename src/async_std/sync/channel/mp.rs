#![allow(unreachable_pub)]

use super::*;
use crate::sync::CachePadded;

struct Slot<T> {
	sequence: AtomicUsize,
	data: UnsafeCell<MaybeUninit<T>>
}

macro_rules! common_impl {
	($channel:ident) => {
		#[asynchronous]
		impl<T> $channel<T> {
			fn next_slot<M, F, I, O>(
				&self, counter: &AtomicUsize, value: I, map: M, success: F
			) -> result::Result<O, I>
			where
				M: Fn(usize) -> (usize, usize),
				F: FnOnce(&Slot<T>, I) -> O
			{
				#[allow(clippy::arithmetic_side_effects)]
				let mask = self.slots.len() - 1;
				let mut pos = counter.load(Ordering::Relaxed);
				let mut slot;

				loop {
					/* Safety: masked */
					slot = unsafe { self.slots.get_unchecked(pos & mask) };

					let (expect, next) = map(pos);
					let sequence = slot.sequence.load(Ordering::Acquire);

					/* capacity can't be greater than isize::MAX */
					#[allow(clippy::cast_possible_wrap)]
					let diff = sequence.wrapping_sub(expect) as isize;

					#[allow(clippy::comparison_chain)]
					if diff == 0 {
						let result = counter.compare_exchange_weak(
							pos,
							pos.wrapping_add(1),
							Ordering::Relaxed,
							Ordering::Relaxed
						);

						if let Err(prev) = result {
							pos = prev;

							continue;
						}

						let value = success(slot, value);

						slot.sequence.store(next, Ordering::Release);

						break Ok(value);
					} else if diff < 0 {
						break Err(value);
					}

					pos = counter.load(Ordering::Relaxed);
				}
			}

			pub fn try_send(&self, value: T) -> result::Result<(), T> {
				let result = self.next_slot(
					&self.tail,
					value,
					|pos| (pos, pos.wrapping_add(1)),
					|slot, value| {
						/* Safety: exclusive access */
						unsafe { ptr!(slot.data=>write(value)) };
					}
				);

				if result.is_ok() {
					self.wake_recv();
				}

				result
			}

			#[allow(clippy::multiple_unsafe_ops_per_block)]
			pub fn try_recv(&self) -> Option<T> {
				let result = self.next_slot(
					&self.head,
					(),
					|pos| (pos.wrapping_add(1), pos.wrapping_add(self.slots.len())),
					/* Safety: exclusive access. this value was initialized earlier */
					|slot, ()| unsafe { ptr!(slot.data=>assume_init_read()) }
				);

				if result.is_ok() {
					self.wake_send();
				}

				result.ok()
			}

			fn can_send(&self) -> bool {
				let head = self.head.load(Ordering::Relaxed);
				let tail = self.tail.load(Ordering::Relaxed);

				tail == head.wrapping_add(self.slots.len())
			}

			fn wake_send(&self) {
				self.tx_waiters.wake_one(());
			}

			pub fn new_sender(&self) {
				self.tx_count.fetch_add(1, Ordering::Relaxed);
			}

			pub fn drop_sender(&self) {
				let prev = self.tx_count.fetch_sub(1, Ordering::Relaxed);

				if prev == 1 {
					self.close_recv();
				}
			}

			pub fn is_send_closed(&self) -> bool {
				self.tx_waiters.is_closed()
			}

			pub fn close_send(&self) {
				self.tx_waiters.close(());
			}

			pub async fn send_notified(&self) {
				let _ = self.tx_waiters.notified(|| !self.can_send()).await;
			}

			pub async fn send_closed(&self) -> result::Result<(), WaitError> {
				self.tx_waiters.notified(|| true).await
			}
		}

		impl<T> Drop for $channel<T> {
			#[allow(clippy::multiple_unsafe_ops_per_block)]
			fn drop(&mut self) {
				let mut head = *self.head.get_mut();
				let tail = *self.tail.get_mut();
				let len = tail.wrapping_sub(head);

				#[allow(clippy::arithmetic_side_effects)]
				let mask = self.slots.len() - 1;

				for _ in 0..len {
					/* Safety: masked */
					let slot = unsafe { self.slots.get_unchecked_mut(head & mask) };

					/* Safety: this slot was initialized */
					unsafe { ptr!(slot.data=>assume_init_drop()) };

					head = head.wrapping_add(1);
				}
			}
		}

		/* Safety: T is send */
		unsafe impl<T: Send> Send for $channel<T> {}

		/* Safety: T is send */
		unsafe impl<T: Send> Sync for $channel<T> {}
	}
}

#[repr(C)]
pub struct MCChannel<T> {
	/* only accessed by receivers */
	head: CachePadded<AtomicUsize>,

	/* only accessed by senders */
	tail: CachePadded<AtomicUsize>,

	/* accessed by both */
	slots: Box<[Slot<T>]>,
	tx_waiters: ThreadSafeWaitList,
	rx_waiters: ThreadSafeWaitList,
	tx_count: AtomicUsize,
	rx_count: AtomicUsize
}

common_impl!(MCChannel);

#[asynchronous]
impl<T> MCChannel<T> {
	#[allow(clippy::expect_used)]
	pub fn new(mut size: usize) -> Self {
		assert!(size != 0, "Cannot create a zero sized channel");

		size = size.checked_next_power_of_two().expect("Channel too big");

		let mut slots = Vec::with_capacity(size);

		for i in 0..size {
			slots.push(Slot {
				sequence: AtomicUsize::new(i),
				data: UnsafeCell::new(MaybeUninit::uninit())
			});
		}

		Self {
			head: CachePadded(AtomicUsize::new(0)),
			tail: CachePadded(AtomicUsize::new(0)),

			slots: slots.into_boxed_slice(),

			tx_waiters: ThreadSafeWaitList::new(),
			rx_waiters: ThreadSafeWaitList::new(),
			tx_count: AtomicUsize::new(0),
			rx_count: AtomicUsize::new(0)
		}
	}

	fn can_recv(&self) -> bool {
		let head = self.head.load(Ordering::Relaxed);
		let tail = self.tail.load(Ordering::Relaxed);

		tail != head
	}

	fn wake_recv(&self) {
		self.rx_waiters.wake_one(());
	}

	pub fn is_recv_closed(&self) -> bool {
		self.rx_waiters.is_closed()
	}

	pub fn close_recv(&self) {
		self.rx_waiters.close(());
	}

	pub fn new_receiver(&self) {
		self.rx_count.fetch_add(1, Ordering::Relaxed);
	}

	pub fn drop_receiver(&self) {
		let prev = self.rx_count.fetch_sub(1, Ordering::Relaxed);

		if prev == 1 {
			self.close_send();
		}
	}

	pub async fn recv_notified(&self) {
		let _ = self.rx_waiters.notified(|| !self.can_recv()).await;
	}
}

pub struct SCChannel<T> {
	/* only accessed by receivers */
	head: CachePadded<AtomicUsize>,

	/* only accessed by senders */
	tail: CachePadded<AtomicUsize>,

	/* accessed by both */
	slots: Box<[Slot<T>]>,
	tx_waiters: ThreadSafeWaitList,
	rx_waiter: AtomicWaiter,
	tx_count: AtomicUsize
}

common_impl!(SCChannel);

#[asynchronous]
impl<T> SCChannel<T> {
	#[allow(clippy::expect_used)]
	pub fn new(mut size: usize) -> Self {
		assert!(size != 0, "Cannot create a zero sized channel");

		size = size.checked_next_power_of_two().expect("Channel too big");

		let mut slots = Vec::with_capacity(size);

		for i in 0..size {
			slots.push(Slot {
				sequence: AtomicUsize::new(i),
				data: UnsafeCell::new(MaybeUninit::uninit())
			});
		}

		Self {
			head: CachePadded(AtomicUsize::new(0)),
			tail: CachePadded(AtomicUsize::new(0)),

			slots: slots.into_boxed_slice(),

			tx_waiters: ThreadSafeWaitList::new(),
			rx_waiter: AtomicWaiter::new(),
			tx_count: AtomicUsize::new(0)
		}
	}

	fn wake_recv(&self) {
		self.rx_waiter.wake(());
	}

	pub fn is_recv_closed(&self) -> bool {
		self.rx_waiter.is_closed()
	}

	pub fn close_recv(&self) {
		self.rx_waiter.close(());
	}

	#[allow(clippy::unused_self)]
	pub const fn new_receiver(&self) {}

	pub fn drop_receiver(&self) {
		self.close_send();
	}

	pub async fn recv_notified(&self) {
		let _ = self.rx_waiter.notified_thread_safe().await;
	}
}

macro_rules! channel_impl {
	($channel:ident) => {
		pub struct Receiver<T> {
			channel: Arc<$channel<T>>
		}

		#[asynchronous]
		impl<T> Receiver<T> {
			fn new(channel: Arc<$channel<T>>) -> Self {
				channel.new_receiver();

				Self { channel }
			}

			pub fn try_recv(&self) -> RecvResult<T> {
				match self.channel.try_recv() {
					Some(value) => Ok(value),
					None => Err(RecvError::new(self.channel.is_recv_closed()))
				}
			}

			pub async fn recv(&self) -> RecvResult<T> {
				loop {
					let result = self.try_recv();

					if !matches!(result, Err(RecvError::Empty)) || is_interrupted().await {
						return result;
					}

					self.channel.recv_notified().await;
				}
			}

			pub fn close(&self) {
				self.channel.close_send();
			}
		}

		impl<T> Drop for Receiver<T> {
			fn drop(&mut self) {
				self.channel.drop_receiver();
			}
		}

		pub struct Sender<T> {
			channel: Arc<$channel<T>>
		}

		#[asynchronous]
		impl<T> Sender<T> {
			fn new(channel: Arc<$channel<T>>) -> Self {
				channel.new_sender();

				Self { channel }
			}

			pub fn try_send(&self, value: T) -> SendResult<T> {
				match self.channel.try_send(value) {
					Ok(()) => Ok(()),
					Err(value) => Err(SendError::new(value, self.channel.is_send_closed()))
				}
			}

			pub async fn send(&self, mut value: T) -> SendResult<T> {
				loop {
					match self.try_send(value) {
						Ok(()) => return Ok(()),
						Err(err @ SendError::Closed(_)) => return Err(err),
						result if is_interrupted().await => return result,
						Err(SendError::Full(v)) => value = v
					}

					self.channel.send_notified().await;
				}
			}

			#[must_use]
			pub fn is_closed(&self) -> bool {
				self.channel.is_send_closed()
			}

			pub async fn closed(&mut self) -> bool {
				let mut closed;

				loop {
					closed = self.is_closed();

					if closed || self.channel.send_closed().await == Err(WaitError::Cancelled) {
						break;
					}
				}

				closed
			}
		}

		impl<T> Clone for Sender<T> {
			fn clone(&self) -> Self {
				Self::new(self.channel.clone())
			}
		}

		impl<T> Drop for Sender<T> {
			fn drop(&mut self) {
				self.channel.drop_sender();
			}
		}

		#[must_use]
		pub fn bounded<T>(size: usize) -> (Sender<T>, Receiver<T>) {
			let channel = Arc::new($channel::new(size));

			(Sender::new(channel.clone()), Receiver::new(channel))
		}
	};
}

pub(super) use channel_impl;
