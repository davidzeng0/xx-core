#![allow(unreachable_pub, dead_code, clippy::module_name_repetitions)]

use std::{
	marker::PhantomData,
	sync::atomic::{AtomicBool, AtomicUsize, Ordering}
};

use super::*;
use crate::{
	container::zero_alloc::linked_list::*,
	coroutines::{block_on, block_on_thread_safe, is_interrupted},
	error::*,
	future::*,
	impls::Cell,
	macros::container_of,
	pointer::*,
	runtime::*,
	sync::{SpinMutex, SpinMutexGuard}
};

struct Waiter<T> {
	node: Node,
	request: ReqPtr<Result<MaybePanic<T>>>
}

impl<T> Waiter<T> {
	unsafe fn complete(node: Ptr<Node>, value: MaybePanic<T>) {
		/* Safety: all nodes are wrapped in Waiter */
		let waiter = unsafe { container_of!(node, Self=>node) };

		/* Safety: the waiter must be valid */
		let request = unsafe { ptr!(waiter=>request) };

		/* Safety: complete the future */
		unsafe { Request::complete(request, Ok(value)) };
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
	unsafe fn wait_notified(&self, waiter: &mut Waiter<T>, request: _) -> Result<MaybePanic<T>> {
		#[cancel]
		fn cancel(waiter: MutPtr<Waiter<T>>, request: _) -> Result<()> {
			/* Safety: guaranteed by future's contract */
			let waiter = unsafe { waiter.as_ref() };

			/* Safety: we linked this node earlier */
			unsafe { waiter.node.unlink_unchecked() };

			/* Safety: send the cancellation. waiter is unlinked, so there won't be
			 * another completion
			 *
			 * note: the waiter may no longer be valid after this call
			 */
			unsafe { Request::complete(waiter.request, Err(Core::interrupted().into())) };

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
		if self.closed.get() || is_interrupted().await {
			return Err(Core::interrupted().into());
		}

		/* we don't really care if it overflows */
		#[allow(clippy::arithmetic_side_effects)]
		self.count.update(|count| count + 1);

		let mut waiter = Waiter { node: Node::new(), request: Ptr::null() };

		/* Safety: waiter is new, pinned, and lives until it completes */
		let result = unsafe { block_on(self.wait_notified(&mut waiter)).await };

		#[allow(clippy::arithmetic_side_effects)]
		self.count.update(|count| count - 1);

		result.map(join)
	}

	pub fn wake_one(&self, value: T) -> bool {
		let Some(node) = self.list.pop_front() else {
			return false;
		};

		/* Safety: complete the future */
		unsafe { Waiter::complete(node, Ok(value)) };

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
					Waiter::complete(node, Ok(value));

					break;
				}

				Waiter::complete(node, catch_unwind_safe(|| value.clone()));
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
		count.fetch_add(1, Ordering::Relaxed);

		Self(count)
	}
}

impl Drop for Counter<'_> {
	fn drop(&mut self) {
		self.0.fetch_sub(1, Ordering::Relaxed);
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
	unsafe fn wait_notified<F>(
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
			unsafe { Request::complete(waiter.request, Err(Core::interrupted().into())) };

			Ok(())
		}

		waiter.request = request;

		let list = self.list();

		if self.closed.load(Ordering::Relaxed) || !should_block() {
			return Progress::Done(Err(Core::interrupted().into()));
		}

		/* Safety: guaranteed by caller */
		unsafe { list.append(&waiter.node) };

		Progress::Pending(cancel(self, ptr!(waiter), request))
	}

	pub async fn notified<F>(&self, should_block: F) -> Result<T>
	where
		F: FnOnce() -> bool
	{
		if is_interrupted().await {
			return Err(Core::interrupted().into());
		}

		let counter = Counter::new(&self.count);
		let mut waiter = Waiter { node: Node::new(), request: Ptr::null() };

		/* Safety: waiter is new, pinned, and lives until it completes */
		let result =
			unsafe { block_on_thread_safe(self.wait_notified(&mut waiter, should_block)).await };

		drop(counter);

		result.map(join)
	}

	pub fn wake_one(&self, value: T) -> bool {
		let Some(node) = self.list().pop_front() else {
			return false;
		};

		/* Safety: complete the future */
		unsafe { Waiter::complete(node, Ok(value)) };

		true
	}

	pub fn wake_all(&self, value: T) -> usize {
		let count = self.count.load(Ordering::Relaxed);
		let list = self.list();

		while let Some(node) = list.pop_front() {
			/*
			 * Safety: complete the future
			 */
			#[allow(clippy::multiple_unsafe_ops_per_block)]
			unsafe {
				if list.is_empty() {
					Waiter::complete(node, Ok(value));

					break;
				}

				Waiter::complete(node, catch_unwind_safe(|| value.clone()));
			};
		}

		count
	}

	pub fn close(&self, value: T) -> usize {
		self.closed.store(true, Ordering::Relaxed);
		self.wake_all(value)
	}
}
