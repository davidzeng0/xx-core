use std::rc::Rc;

use super::*;
use crate::{
	container::zero_alloc::linked_list::*, container_of, coroutines::block_on, error::*, future::*,
	pointer::*
};

struct Waiter {
	node: Node,
	request: ReqPtr<Result<()>>
}

pub struct Notify {
	waiters: LinkedList
}

#[asynchronous]
impl Notify {
	/// Safety: caller must
	/// - pin this Notify
	/// - call Notify::pin
	/// - never move this datta
	pub unsafe fn new_unpinned() -> Self {
		Self { waiters: LinkedList::new() }
	}

	pub fn new() -> Pinned<Rc<Self>> {
		/* Safety: Self cannot be unpinned */
		unsafe { Self::new_unpinned() }.pin_rc()
	}

	#[future]
	fn wait_notified(&self, waiter: &mut Waiter) -> Result<()> {
		fn cancel(waiter: &Waiter) -> Result<()> {
			/* Safety: our linked list is always in a consistent state */
			unsafe {
				waiter.node.unlink();

				Request::complete(waiter.request, Err(Core::Interrupted.new()));
			}

			Ok(())
		}

		waiter.request = request;

		/* Safety: our linked list is always in a consistent state */
		unsafe { self.waiters.append(&waiter.node) };

		Progress::Pending(cancel(waiter, request))
	}

	pub async fn notified(&self) -> Result<()> {
		let mut waiter = Waiter { node: Node::new(), request: Ptr::null() };

		block_on(self.wait_notified(&mut waiter)).await
	}

	pub fn notify(&self) {
		/* Safety: our linked list is always in a consistent state */
		unsafe {
			let mut list = LinkedList::new();
			let mut list = list.pin_local();

			self.waiters.move_elements(&mut list);

			while !list.empty() {
				let head = list.head();
				let waiter = container_of!(head, Waiter:node).as_ref();

				head.as_ref().unlink();

				Request::complete(waiter.request, Ok(()));
			}
		}
	}
}

unsafe impl Pin for Notify {
	unsafe fn pin(&mut self) {
		self.waiters.pin();
	}
}
