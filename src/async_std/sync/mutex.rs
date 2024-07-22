#![allow(clippy::module_name_repetitions)]

use std::hint::spin_loop;
use std::ops::{Deref, DerefMut};
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::{fmt, result};

use super::*;
use crate::sync::poison::*;

#[errors(?Debug + ?Display)]
pub enum LockError<T> {
	#[display(transparent)]
	Poisoned(#[from] PoisonError<T>),

	#[display("Lock failed: the operation would block and the current task is interrupted")]
	#[kind = ErrorKind::Interrupted]
	Interrupted
}

pub type LockResult<T> = result::Result<T, LockError<T>>;

pub struct MutexGuard<'a, T: ?Sized> {
	lock: &'a Mutex<T>,
	poison: PoisonGuard<'a>
}

impl<'a, T: ?Sized> MutexGuard<'a, T> {
	/// # Safety
	/// must create atmost 1 guards after a successful locking of the mutex
	unsafe fn new(lock: &'a Mutex<T>) -> Self {
		Self { lock, poison: lock.poison.guard() }
	}
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		/* Safety: lock held */
		unsafe { self.lock.value.as_ref() }
	}
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		/* Safety: lock held */
		unsafe { self.lock.value.as_mut() }
	}
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
	fn drop(&mut self) {
		self.poison.finish();

		/* Safety: we own the lock */
		unsafe { self.lock.unlock() };
	}
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for MutexGuard<'_, T> {
	fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
		(**self).fmt(fmt)
	}
}

/* Safety: same as &mut T */
unsafe impl<T: ?Sized + Send> Send for MutexGuard<'_, T> {}

/* Safety: same as &mut T */
unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum State {
	Unlocked,
	Locked,
	Contended
}

pub struct Mutex<T: ?Sized> {
	state: AtomicU8,
	wait_list: ThreadSafeWaitList<()>,
	poison: PoisonFlag,
	value: UnsafeCell<T>
}

#[asynchronous]
impl<T: ?Sized> Mutex<T> {
	pub fn new(value: T) -> Self
	where
		T: Sized
	{
		Self {
			state: AtomicU8::new(State::Unlocked as u8),
			wait_list: ThreadSafeWaitList::new(),
			poison: PoisonFlag::new(),
			value: UnsafeCell::new(value)
		}
	}

	/// # Safety
	/// must have acquired the lock
	unsafe fn unlock(&self) {
		let state = self.state.swap(State::Unlocked as u8, Ordering::SeqCst);

		if state == State::Contended as u8 {
			self.wait_list.wake_one(());
		}
	}

	fn try_lock_internal(&self) -> bool {
		self.state
			.compare_exchange(
				State::Unlocked as u8,
				State::Locked as u8,
				Ordering::Acquire,
				Ordering::Relaxed
			)
			.is_ok()
	}

	fn try_spin_lock(&self) -> u8 {
		let mut state = State::Locked as u8;

		for _ in 0..97 {
			state = self.state.load(Ordering::Relaxed);

			if state != State::Locked as u8 {
				break;
			}

			spin_loop();
		}

		if state != State::Contended as u8 {
			state = self.state.swap(State::Contended as u8, Ordering::Acquire);
		}

		state
	}

	#[cold]
	async fn lock_contended(&self) -> bool {
		loop {
			let prev_state = self.try_spin_lock();

			if prev_state == State::Unlocked as u8 {
				break true;
			}

			if is_interrupted().await {
				break false;
			}

			let should_block = || self.state.load(Ordering::SeqCst) == State::Contended as u8;
			let _ = self.wait_list.wait(should_block).await;
		}
	}

	/// # Panics
	/// If the lock needs to wait and current worker is interrupted
	pub async fn lock(&self) -> LockResult<MutexGuard<'_, T>> {
		if !self.try_lock_internal() {
			let locked = self.lock_contended().await;

			if !locked {
				return Err(LockError::Interrupted);
			}
		}

		/* Safety: guaranteed by caller */
		let guard = unsafe { MutexGuard::new(self) };

		self.poison.map(guard).map_err(Into::into)
	}

	pub fn try_lock(&self) -> TryLockResult<MutexGuard<'_, T>> {
		if self.try_lock_internal() {
			/* Safety: guaranteed by caller */
			let guard = unsafe { MutexGuard::new(self) };

			Ok(self.poison.map(guard)?)
		} else {
			Err(TryLockError::WouldBlock)
		}
	}

	pub fn is_poisoned(&self) -> bool {
		self.poison.get()
	}

	pub fn clear_poison(&self) {
		self.poison.clear();
	}

	pub fn into_inner(self) -> LockResult<T>
	where
		T: Sized
	{
		self.poison.map(self.value.into_inner()).map_err(Into::into)
	}

	pub fn get_mut(&mut self) -> LockResult<&mut T> {
		self.poison.map(self.value.get_mut()).map_err(Into::into)
	}
}

/* Safety: T is send */
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}

/* Safety: T is send. mutex provides synchronization */
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

impl<T: ?Sized> UnwindSafe for Mutex<T> {}

impl<T: ?Sized> RefUnwindSafe for Mutex<T> {}
