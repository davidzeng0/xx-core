#![allow(unreachable_pub, dead_code, clippy::module_name_repetitions)]

use std::{
	marker::PhantomData,
	sync::atomic::{Ordering::*, *}
};

use super::*;
use crate::{
	container::zero_alloc::linked_list::*,
	coroutines::block_on,
	future::*,
	impls::Cell,
	macros::container_of,
	pointer::AtomicPtr,
	runtime::{catch_unwind_safe, join, MaybePanic},
	sync::{SpinMutex, SpinMutexGuard}
};

#[errors]
pub enum WaitError {
	#[error("Suspend cancelled")]
	#[kind = ErrorKind::Interrupted]
	Cancelled,

	#[error("Wait list closed")]
	Closed
}

const fn closed<T>() -> ReqPtr<T> {
	Ptr::from_addr(usize::MAX)
}

pub struct AtomicWaiter<T = ()> {
	waiter: AtomicPtr<Request<Result<T>>>
}

#[asynchronous]
impl<T> AtomicWaiter<T> {
	#[must_use]
	pub const fn new() -> Self {
		Self { waiter: AtomicPtr::new(ReqPtr::null()) }
	}

	fn set_waiter(&self, request: ReqPtr<Result<T>>) -> ReqPtr<Result<T>> {
		let result = self.waiter.fetch_update(Relaxed, Relaxed, |prev| {
			(prev != closed()).then_some(request)
		});

		result.unwrap_or_else(|err| err)
	}

	#[future]
	fn wait(&self, request: _) -> Result<T> {
		#[cancel]
		fn cancel(&self, request: _) -> Result<()> {
			/* wake may already be in progress */
			let result = self
				.waiter
				.compare_exchange(request, Ptr::null(), Relaxed, Relaxed);

			if result.is_ok() {
				/* Safety: we took ownership of waking up the task. send the cancellation */
				unsafe { Request::complete(request, Err(ErrorKind::Interrupted.into())) };
			}

			Ok(())
		}

		let prev = self.set_waiter(request);

		if !prev.is_null() {
			if prev == closed() {
				return Progress::Done(Err(WaitError::Closed.into()));
			}

			/* Safety: we took ownership of waking up the task. send the cancellation */
			unsafe { Request::complete(prev, Err(WaitError::Cancelled.into())) };
		}

		Progress::Pending(cancel(self, request))
	}

	pub async fn notified(&self) -> Result<T> {
		check_interrupt().await?;
		block_on(self.wait()).await
	}

	pub async fn notified_thread_safe(&self) -> Result<T> {
		check_interrupt().await?;
		block_on_thread_safe(self.wait()).await
	}

	fn wake_internal(&self, new_waiter: ReqPtr<Result<T>>, value: T) -> bool {
		let prev = self.set_waiter(new_waiter);

		if prev.is_null() || prev == closed() {
			return false;
		}

		/* Safety: complete the future */
		unsafe { Request::complete(prev, Ok(value)) };

		true
	}

	pub fn wake(&self, value: T) -> bool {
		self.wake_internal(Ptr::null(), value)
	}

	pub fn close(&self, value: T) -> bool {
		self.wake_internal(closed(), value)
	}

	pub fn is_closed(&self) -> bool {
		self.waiter.load(Relaxed) == closed()
	}
}

struct Waiter<T> {
	node: Node,
	request: ReqPtr<Result<MaybePanic<T>>>
}

impl<T> Waiter<T> {
	unsafe fn complete(this: Ptr<Self>, value: Result<MaybePanic<T>>) {
		/* Safety: guaranteed by caller */
		let request = unsafe { ptr!(this=>request) };

		/* Safety: complete the future */
		unsafe { Request::complete(request, value) };
	}

	unsafe fn complete_node(node: Ptr<Node>, value: Result<MaybePanic<T>>) {
		/* Safety: guaranteed by caller */
		let waiter = unsafe { container_of!(node, Self=>node) };

		/* Safety: guaranteed by caller */
		unsafe { Self::complete(waiter, value) }
	}
}

pub struct RawWaitList<T = ()> {
	list: LinkedList,
	count: Cell<usize>,
	closed: Cell<bool>,
	phantom: PhantomData<T>
}

#[asynchronous]
impl<T: Clone> RawWaitList<T> {
	#[must_use]
	pub const unsafe fn new() -> Self {
		Self {
			list: LinkedList::new(),
			count: Cell::new(0),
			closed: Cell::new(false),
			phantom: PhantomData
		}
	}

	/// # Safety
	/// Waiter must be pinned, unlinked, and live until it's waked
	#[future]
	unsafe fn wait(&self, waiter: &mut Waiter<T>, request: _) -> Result<MaybePanic<T>> {
		#[cancel]
		fn cancel(waiter: MutPtr<Waiter<T>>, request: _) -> Result<()> {
			/* Safety: we linked this node earlier */
			#[allow(clippy::multiple_unsafe_ops_per_block)]
			(unsafe { ptr!(waiter=>node.unlink_unchecked()) });

			/* Safety: send the cancellation. waiter is unlinked, so there won't be
			 * another completion
			 *
			 * note: the waiter may no longer be valid after this call
			 */
			unsafe { Waiter::complete(waiter.cast_const(), Err(ErrorKind::Interrupted.into())) };

			Ok(())
		}

		waiter.request = request;

		/* Safety: guaranteed by caller
		 *
		 * note: even though we have &mut to the Waiter, and &mut Cell<T> by
		 * extension, it's not actually UB if the node's pointers get changed
		 * as long as we don't call `Cell::get_mut`
		 */
		unsafe { self.list.append(&waiter.node) };

		Progress::Pending(cancel(ptr!(waiter), request))
	}

	pub async fn notified(&self) -> Result<T> {
		if self.closed.get() {
			return Err(WaitError::Closed.into());
		}

		check_interrupt().await?;

		/* we don't really care if it overflows */
		#[allow(clippy::arithmetic_side_effects)]
		self.count.update(|count| count + 1);

		let mut waiter = Waiter { node: Node::new(), request: Ptr::null() };

		/* Safety: waiter is new, pinned, and lives until it completes */
		let result = block_on(unsafe { self.wait(&mut waiter) }).await;

		#[allow(clippy::arithmetic_side_effects)]
		self.count.update(|count| count - 1);

		result.map(join)
	}

	pub fn wake_one(&self, value: T) -> bool {
		let Some(node) = self.list.pop_front() else {
			return false;
		};

		/* Safety: complete the future */
		unsafe { Waiter::complete_node(node, Ok(Ok(value))) };

		true
	}

	pub fn wake_all(&self, value: T) -> usize {
		let mut list = LinkedList::new();
		let list = list.pin_local();
		let count = self.count.get();

		/* Safety: our new list is pinned, and we clear out all nodes before
		 * returning
		 */
		unsafe { self.list.move_elements(&list) };

		while let Some(node) = list.pop_front() {
			/*
			 * Safety: complete the future
			 */
			#[allow(clippy::multiple_unsafe_ops_per_block)]
			unsafe {
				if list.is_empty() {
					Waiter::complete_node(node, Ok(Ok(value)));

					break;
				}

				Waiter::complete_node(node, Ok(catch_unwind_safe(|| value.clone())));
			};
		}

		count
	}

	pub fn close(&self, value: T) -> usize {
		self.closed.set(true);
		self.wake_all(value)
	}
}

impl<T> Pin for RawWaitList<T> {
	unsafe fn pin(&mut self) {
		/* Safety: we are being pinned */
		unsafe { self.list.pin() };
	}
}

pub struct ThreadSafeWaitList<T = ()> {
	list: SpinMutex<Pinned<Box<LinkedList>>>,
	count: AtomicUsize,
	closed: AtomicBool,
	phantom: PhantomData<T>
}

struct Counter<'a>(&'a AtomicUsize);

impl<'a> Counter<'a> {
	pub fn new(count: &'a AtomicUsize) -> Self {
		count.fetch_add(1, Relaxed);

		Self(count)
	}
}

impl Drop for Counter<'_> {
	fn drop(&mut self) {
		self.0.fetch_sub(1, Relaxed);
	}
}

#[asynchronous]
impl<T: Clone> ThreadSafeWaitList<T> {
	#[must_use]
	pub fn new() -> Self {
		Self {
			list: SpinMutex::new(LinkedList::new().pin_box()),
			count: AtomicUsize::new(0),
			closed: AtomicBool::new(false),
			phantom: PhantomData
		}
	}

	fn list(&self) -> SpinMutexGuard<'_, Pinned<Box<LinkedList>>> {
		match self.list.lock() {
			Ok(list) => list,
			Err(err) => {
				self.list.clear_poison();

				err.into_inner()
			}
		}
	}

	/// # Safety
	/// Waiter must be pinned, unlinked, and live until it's waked
	#[future]
	unsafe fn wait<F>(
		&self, waiter: &mut Waiter<T>, should_block: F, request: _
	) -> Result<MaybePanic<T>>
	where
		F: FnOnce() -> bool
	{
		#[cancel]
		fn cancel(&self, waiter: MutPtr<Waiter<T>>, request: _) -> Result<()> {
			#[allow(clippy::unwrap_used)]
			let list = self.list();

			/* Safety: guaranteed by future's contract */
			let waiter = unsafe { waiter.as_ref() };

			/* the node may have already been unlinked */
			if !waiter.node.linked() {
				return Ok(());
			}

			/* Safety: the node hasn't been unlinked yet */
			unsafe { waiter.node.unlink() };

			drop(list);

			/* Safety: send the cancellation. waiter is unlinked, so there won't be
			 * another completion
			 *
			 * note: the waiter may no longer be valid after this call
			 */
			unsafe { Request::complete(waiter.request, Err(ErrorKind::Interrupted.into())) };

			Ok(())
		}

		waiter.request = request;

		let list = self.list();

		if !should_block() {
			return Progress::Done(Err(WaitError::Cancelled.into()));
		}

		if self.closed.load(Relaxed) {
			return Progress::Done(Err(WaitError::Closed.into()));
		}

		/* Safety: guaranteed by caller */
		unsafe { list.append(&waiter.node) };

		Progress::Pending(cancel(self, ptr!(waiter), request))
	}

	pub async fn notified<F>(&self, should_block: F) -> Result<T>
	where
		F: FnOnce() -> bool
	{
		check_interrupt().await?;

		let counter = Counter::new(&self.count);
		let mut waiter = Waiter { node: Node::new(), request: Ptr::null() };

		/* Safety: waiter is new, pinned, and lives until it completes */
		let result = block_on_thread_safe(unsafe { self.wait(&mut waiter, should_block) }).await;

		drop(counter);

		result.map(join)
	}

	pub fn wake_one(&self, value: T) -> bool {
		let Some(node) = self.list().pop_front() else {
			return false;
		};

		/* Safety: complete the future */
		unsafe { Waiter::complete_node(node, Ok(Ok(value))) };

		true
	}

	pub fn wake_all(&self, value: T) -> usize {
		let count = self.count.load(Relaxed);
		let list = self.list();

		while let Some(node) = list.pop_front() {
			/*
			 * Safety: complete the future
			 */
			#[allow(clippy::multiple_unsafe_ops_per_block)]
			unsafe {
				if list.is_empty() {
					Waiter::complete_node(node, Ok(Ok(value)));

					break;
				}

				Waiter::complete_node(node, Ok(catch_unwind_safe(|| value.clone())));
			};
		}

		count
	}

	pub fn close(&self, value: T) -> usize {
		self.closed.store(true, Relaxed);
		self.wake_all(value)
	}
}