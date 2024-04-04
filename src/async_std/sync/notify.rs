use std::{marker::PhantomData, rc::Rc};

use super::*;
use crate::{
	container::zero_alloc::linked_list::*,
	coroutines::{block_on, is_interrupted},
	error::*,
	future::*,
	macros::container_of,
	pointer::*
};

struct Waiter<T> {
	node: Node,
	request: ReqPtr<Result<T>>
}

pub struct Notify<T = ()> {
	waiters: LinkedList,
	phantom: PhantomData<T>
}

#[asynchronous]
impl<T: Clone> Notify<T> {
	/// # Safety
	/// caller must
	/// - pin this Notify
	/// - call Notify::pin
	/// # Unpin
	/// only if waiters is empty
	#[must_use]
	pub const unsafe fn new_unpinned() -> Self {
		Self { waiters: LinkedList::new(), phantom: PhantomData }
	}

	#[must_use]
	pub fn new() -> Pinned<Rc<Self>> {
		/* Safety: Self cannot be unpinned */
		unsafe { Self::new_unpinned() }.pin_rc()
	}

	/// # Safety
	/// Waiter must be pinned, unlinked, and live as long as it is linked
	#[future]
	unsafe fn wait_notified(&self, waiter: &mut Waiter<T>) -> Result<T> {
		#[cancel]
		fn cancel(waiter: &Waiter<T>) -> Result<()> {
			/* Safety: we linked this node earlier */
			unsafe { waiter.node.unlink_unchecked() };

			/* Safety: send the cancellation. waiter is unlinked, so there won't be
			 * another completion */
			unsafe { Request::complete(waiter.request, Err(Core::interrupted().into())) };

			Ok(())
		}

		waiter.request = request;

		/* Safety: guaranteed by caller. we don't mutably borrow waiter anymore */
		unsafe { self.waiters.append(&waiter.node) };

		Progress::Pending(cancel(waiter, request))
	}

	pub async fn notified(&self) -> Result<T> {
		if is_interrupted().await {
			return Err(Core::interrupted().into());
		}

		let mut waiter = Waiter { node: Node::new(), request: Ptr::null() };

		/* Safety: waiter is new, pinned, and lives until it completes */
		#[allow(clippy::multiple_unsafe_ops_per_block)]
		unsafe {
			block_on(self.wait_notified(&mut waiter)).await
		}
	}

	pub fn notify(&self, value: T) {
		let mut list = LinkedList::new();
		let list = list.pin_local();

		/* Safety: our new list is pinned, and we clear out all nodes before
		 * returning */
		unsafe { self.waiters.move_elements(&list) };

		while let Some(node) = list.pop_front() {
			let waiter = container_of!(node, Waiter<T>:node);

			/* Safety: all nodes are wrapped in Waiter */
			let request = unsafe { waiter.as_ref() }.request;

			/*
			 * Safety: complete the future
			 * Note: this cannot panic
			 */
			unsafe { Request::complete(request, Ok(value.clone())) };
		}
	}
}

impl<T> Pin for Notify<T> {
	unsafe fn pin(&mut self) {
		/* Safety: we are being pinned */
		unsafe { self.waiters.pin() };
	}
}
