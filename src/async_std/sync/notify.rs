use std::rc::Rc;

use super::*;
use crate::{
	container::zero_alloc::linked_list::*, container_of, coroutines::block_on, error::*,
	pointer::*, task::*
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
	pub unsafe fn new_unpinned() -> Self {
		Self { waiters: LinkedList::new() }
	}

	pub fn new() -> Pinned<Rc<Self>> {
		unsafe { Self::new_unpinned() }.pin_rc()
	}

	#[future]
	fn wait_notified(&self, waiter: &mut Waiter) -> Result<()> {
		fn cancel(waiter: Ptr<Waiter>) -> Result<()> {
			unsafe {
				waiter.node.unlink();

				Request::complete(waiter.request, Err(Core::Interrupted.new()));
			}

			Ok(())
		}

		waiter.request = request;

		unsafe { self.waiters.append(&waiter.node) };

		Progress::Pending(cancel(waiter.into(), request))
	}

	pub async fn notified(&self) -> Result<()> {
		let mut waiter = Waiter { node: Node::new(), request: Ptr::null() };

		block_on(self.wait_notified(&mut waiter)).await
	}

	pub fn notify(&self) {
		unsafe {
			let mut list = LinkedList::new();
			let mut list = list.pin_local();

			self.waiters.move_elements(&mut list);

			while !list.empty() {
				let head = list.head();
				let waiter = container_of!(head, Waiter, node);

				head.unlink();

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
