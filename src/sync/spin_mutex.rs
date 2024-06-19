#![allow(clippy::module_name_repetitions)]

use std::{
	fmt::*,
	ops::{Deref, DerefMut},
	panic::*,
	sync::*
};

use super::*;
use crate::pointer::UnsafeCell;

pub struct SpinMutexGuard<'a, T: ?Sized> {
	lock: &'a SpinMutex<T>,
	poison: PoisonGuard<'a>
}

impl<'a, T: ?Sized> SpinMutexGuard<'a, T> {
	unsafe fn new(lock: &'a SpinMutex<T>) -> Self {
		Self { lock, poison: lock.poison.guard() }
	}
}

impl<T: ?Sized> Deref for SpinMutexGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		/* Safety: lock held */
		unsafe { self.lock.value.as_ref() }
	}
}

impl<T: ?Sized> DerefMut for SpinMutexGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		/* Safety: lock held */
		unsafe { self.lock.value.as_mut() }
	}
}

impl<T: ?Sized> Drop for SpinMutexGuard<'_, T> {
	fn drop(&mut self) {
		self.poison.finish();
		self.lock.lock.unlock();
	}
}

impl<T: ?Sized + Debug> Debug for SpinMutexGuard<'_, T> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
		self.deref().fmt(fmt)
	}
}

/* Safety: same as &mut T */
unsafe impl<T: ?Sized + Send> Send for SpinMutexGuard<'_, T> {}

/* Safety: same as &mut T */
unsafe impl<T: ?Sized + Sync> Sync for SpinMutexGuard<'_, T> {}

pub struct SpinMutex<T: ?Sized> {
	lock: SpinLock,
	poison: PoisonFlag,
	value: UnsafeCell<T>
}

impl<T: ?Sized> SpinMutex<T> {
	pub const fn new(value: T) -> Self
	where
		T: Sized
	{
		Self {
			lock: SpinLock::new(),
			poison: PoisonFlag::new(),
			value: UnsafeCell::new(value)
		}
	}

	pub fn lock(&self) -> LockResult<SpinMutexGuard<'_, T>> {
		self.lock.lock();

		/* Safety: guaranteed by caller */
		let guard = unsafe { SpinMutexGuard::new(self) };

		self.poison.map(guard)
	}

	pub fn try_lock(&self) -> TryLockResult<SpinMutexGuard<'_, T>> {
		if self.lock.try_lock() {
			/* Safety: guaranteed by caller */
			let guard = unsafe { SpinMutexGuard::new(self) };

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
		self.poison.map(self.value.into_inner())
	}

	pub fn get_mut(&mut self) -> LockResult<&mut T> {
		self.poison.map(self.value.get_mut())
	}
}

/* Safety: a mutex is send if T is send */
unsafe impl<T: ?Sized + Send> Send for SpinMutex<T> {}

/* Safety: a mutex is sync if T is send */
unsafe impl<T: ?Sized + Send> Sync for SpinMutex<T> {}

impl<T: ?Sized> UnwindSafe for SpinMutex<T> {}

impl<T: ?Sized> RefUnwindSafe for SpinMutex<T> {}
