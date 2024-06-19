#![allow(clippy::module_name_repetitions)]

use std::{marker::PhantomData, rc::Rc};

use super::*;
use crate::macros::wrapper_functions;

pub struct RawNotify<T = ()> {
	waiters: RawWaitList<T>,
	phantom: PhantomData<T>
}

impl<T: Clone> RawNotify<T> {
	#[asynchronous]
	pub async fn notified(&self) -> Result<T> {
		self.waiters.notified().await.map_err(Into::into)
	}

	pub fn notify(&self, value: T) -> usize {
		self.waiters.wake_all(value)
	}

	/// # Safety
	/// caller must
	/// - pin this Notify
	/// - call Notify::pin
	/// # Unpin
	/// only if waiters is empty
	#[must_use]
	pub const unsafe fn new_unpinned() -> Self {
		Self {
			/* Safety: guaranteed by caller */
			waiters: unsafe { RawWaitList::new() },
			phantom: PhantomData
		}
	}
}

impl<T> Pin for RawNotify<T> {
	unsafe fn pin(&mut self) {
		/* Safety: we are being pinned */
		unsafe { self.waiters.pin() };
	}
}

pub struct RcNotify<T = ()>(Pinned<Rc<RawNotify<T>>>);

impl<T> Clone for RcNotify<T> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<T: Clone> RcNotify<T> {
	wrapper_functions! {
		inner = self.0;

		#[asynchronous]
		pub async fn notified(&self) -> Result<T>;

		pub fn notify(&self, value: T) -> usize;
	}

	#[must_use]
	#[allow(clippy::new_without_default)]
	pub fn new() -> Self {
		/* Safety: cannot be unpinned */
		let raw = unsafe { RawNotify::new_unpinned() };

		Self(raw.pin_rc())
	}
}

pub struct Notify<T = ()> {
	waiters: ThreadSafeWaitList<T>,
	phantom: PhantomData<T>
}

impl<T: Clone> Notify<T> {
	#[must_use]
	pub fn new() -> Self {
		Self {
			/* Safety: guaranteed by caller */
			waiters: ThreadSafeWaitList::new(),
			phantom: PhantomData
		}
	}

	#[asynchronous]
	pub async fn notified(&self) -> Result<T> {
		self.waiters.notified(|| true).await.map_err(Into::into)
	}

	pub fn notify(&self, value: T) -> usize {
		self.waiters.wake_all(value)
	}
}

impl<T: Clone> Default for Notify<T> {
	fn default() -> Self {
		Self::new()
	}
}

/* Safety: T is send */
unsafe impl<T: Send> Send for Notify<T> {}

/* Safety: T is send */
unsafe impl<T: Send> Sync for Notify<T> {}
