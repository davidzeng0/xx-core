#![allow(unreachable_pub)]

use super::*;

struct Slot<T> {
	sequence: AtomicUsize,
	data: UnsafeCell<MaybeUninit<T>>
}

#[allow(clippy::expect_used)]
fn create_slots<T>(mut size: usize) -> Box<[Slot<T>]> {
	assert!(size != 0, "Cannot create a zero sized channel");

	size = size.checked_next_power_of_two().expect("Channel too big");

	let mut slots = Vec::with_capacity(size);

	for i in 0..size {
		slots.push(Slot {
			sequence: AtomicUsize::new(i),
			data: UnsafeCell::new(MaybeUninit::uninit())
		});
	}

	slots.into_boxed_slice()
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
				let mut pos;
				let mut slot;
				let mut backoff = Backoff::new();

				loop {
					pos = counter.load(Ordering::Relaxed);
					/* Safety: masked */
					slot = unsafe { self.slots.get_unchecked(pos & mask) };

					let (expect, next) = map(pos);
					let sequence = slot.sequence.load(Ordering::Acquire);

					/* capacity can't be greater than isize::MAX */
					#[allow(clippy::cast_possible_wrap)]
					let diff = sequence.wrapping_sub(expect) as isize;

					if diff == 0 {
						let result = counter.compare_exchange_weak(
							pos,
							pos.wrapping_add(1),
							Ordering::SeqCst,
							Ordering::Relaxed
						);

						if result.is_err() {
							backoff.spin();

							continue;
						}

						let value = success(slot, value);

						slot.sequence.store(next, Ordering::Release);

						break Ok(value);
					}

					if diff < 0 {
						break Err(value);
					}

					backoff.spin();
				}
			}

			pub fn try_send(&self, value: T) -> result::Result<(), T> {
				self.next_slot(
					&self.tail,
					value,
					|pos| (pos, pos.wrapping_add(1)),
					|slot, value| {
						self.wake_recv();

						/* Safety: exclusive access */
						unsafe { ptr!(slot.data=>write(value)) };
					}
				)
			}

			#[allow(clippy::multiple_unsafe_ops_per_block)]
			pub fn try_recv(&self) -> Option<T> {
				self.next_slot(
					&self.head,
					(),
					|pos| (pos.wrapping_add(1), pos.wrapping_add(self.slots.len())),
					|slot, ()| {
						self.wake_send();

						/* Safety: exclusive access. this value was initialized earlier */
						unsafe { ptr!(slot.data=>assume_init_read()) }
					}
				).ok()
			}

			pub fn len(&self) -> usize {
				let head = self.head.load(Ordering::SeqCst);
				let tail = self.tail.load(Ordering::SeqCst);

				tail.wrapping_sub(head)
			}

			pub fn spare_capacity(&self) -> usize {
				let tail = self.tail.load(Ordering::SeqCst);
				let head = self.head.load(Ordering::SeqCst);

				head.wrapping_add(self.slots.len()).wrapping_sub(tail)
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

			pub async fn send_wait(&self) {
				let _ = self.tx_waiters.wait(|| self.spare_capacity() == 0).await;
			}

			pub async fn send_closed(&self) -> result::Result<(), WaitError> {
				self.tx_waiters.wait(|| true).await
			}

			#[allow(clippy::multiple_unsafe_ops_per_block)]
			fn drop_all(&mut self) {
				let mut head = *self.head.get_mut();
				let tail = *self.tail.get_mut();

				#[allow(clippy::arithmetic_side_effects)]
				let mask = self.slots.len() - 1;

				while head != tail {
					/* Safety: masked */
					let slot = unsafe { self.slots.get_unchecked_mut(head & mask) };

					head = head.wrapping_add(1);

					*self.head.get_mut() = head;

					/* Safety: this slot was initialized */
					unsafe { ptr!(slot.data=>assume_init_drop()) };
				}
			}
		}

		impl<T> Drop for $channel<T> {
			fn drop(&mut self) {
				struct Guard<'a, T> {
					this: &'a mut $channel<T>
				}

				impl<T> Drop for Guard<'_, T> {
					fn drop(&mut self) {
						self.this.drop_all();
					}
				}

				let guard = Guard { this: self };

				guard.this.drop_all();

				forget(guard);
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
	pub fn new(size: usize) -> Self {
		Self {
			head: CachePadded(AtomicUsize::new(0)),
			tail: CachePadded(AtomicUsize::new(0)),

			slots: create_slots(size),
			tx_waiters: ThreadSafeWaitList::new(),
			rx_waiters: ThreadSafeWaitList::new(),
			tx_count: AtomicUsize::new(0),
			rx_count: AtomicUsize::new(0)
		}
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

	pub async fn recv_wait(&self) {
		let _ = self.rx_waiters.wait(|| self.len() == 0).await;
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
				let mut backoff = Backoff::new();

				loop {
					let result = self.try_recv();

					if !matches!(result, Err(RecvError::Empty)) || is_interrupted().await {
						return result;
					}

					if backoff.is_completed() {
						self.channel.recv_wait().await;

						backoff.reset();
					} else {
						backoff.snooze();
					}
				}
			}

			#[must_use]
			pub fn len(&self) -> usize {
				self.channel.len()
			}

			#[must_use]
			pub fn is_empty(&self) -> bool {
				self.len() == 0
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
				let mut backoff = Backoff::new();

				loop {
					match self.try_send(value) {
						Ok(()) => return Ok(()),
						Err(err @ SendError::Closed(_)) => return Err(err),
						result if is_interrupted().await => return result,
						Err(SendError::Full(v)) => value = v
					}

					if backoff.is_completed() {
						self.channel.send_wait().await;

						backoff.reset();
					} else {
						backoff.snooze();
					}
				}
			}

			#[must_use]
			pub fn spare_capacity(&self) -> usize {
				self.channel.spare_capacity()
			}

			#[must_use]
			pub fn is_full(&self) -> bool {
				self.spare_capacity() == 0
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
