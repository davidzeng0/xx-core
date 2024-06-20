#![allow(clippy::multiple_unsafe_ops_per_block)]

use std::cell::Cell;

use super::*;
use crate::macros::assert_unsafe_precondition;
use crate::opt::hint::assume;

pub struct Node {
	prev: Cell<Ptr<Self>>,
	next: Cell<Ptr<Self>>
}

impl Node {
	/// # Safety
	/// self and next are pinned must live as long as they are linked
	unsafe fn set_next(&self, next: &Self) {
		self.next.set(ptr!(next));

		next.prev.set(ptr!(self));
	}

	/// # Safety
	/// self and prev are pinned must live as long as they are linked
	unsafe fn set_prev(&self, prev: &Self) {
		self.prev.set(ptr!(prev));

		prev.next.set(ptr!(self));
	}

	/// # Safety
	/// Combination of `set_next` and `set_prev`
	unsafe fn set_ptrs(&self, prev: &Self, next: &Self) {
		/* Safety: guaranteed by caller */
		unsafe {
			self.set_prev(prev);
			self.set_next(next);
		}
	}

	/// # Safety
	/// Must be pinned and not be linked
	unsafe fn set_circular(&self) {
		/* Safety: guaranteed by caller */
		unsafe { self.set_next(self) };
	}

	#[must_use]
	pub const fn new() -> Self {
		Self {
			prev: Cell::new(Ptr::null()),
			next: Cell::new(Ptr::null())
		}
	}

	pub fn linked(&self) -> bool {
		/*
		 * Safety: if we are linked, both must be non-null, or both must be null
		 * May help with optimizations
		 */
		unsafe { assume(self.prev.get().is_null() == self.next.get().is_null()) };

		!self.prev.get().is_null()
	}

	/// # Safety
	/// This node must be valid and linked
	pub unsafe fn unlink_unchecked(&self) {
		/* Safety: guaranteed by caller */
		unsafe { assert_unsafe_precondition!(self.linked()) };

		let (prev, next) = (
			self.prev.replace(Ptr::null()),
			self.next.replace(Ptr::null())
		);

		/* Safety: prev and next must live as long as they are linked */
		unsafe {
			ptr!(prev=>next.set(next));
			ptr!(next=>prev.set(prev));
		}
	}

	/// # Safety
	/// This node must be valid
	///
	/// # Panics
	/// If this node isn't linked
	pub unsafe fn unlink(&self) {
		assert!(self.linked());

		/* Safety: guaranteed by caller. we just ensured we are linked */
		unsafe { self.unlink_unchecked() };
	}
}

impl Default for Node {
	fn default() -> Self {
		Self::new()
	}
}

pub struct LinkedList {
	base: Node
}

impl LinkedList {
	fn base(&self) -> Ptr<Node> {
		ptr!(&self.base)
	}

	#[must_use]
	pub const fn new() -> Self {
		Self { base: Node::new() }
	}

	pub fn is_empty(&self) -> bool {
		self.base() == self.head()
	}

	pub fn head(&self) -> Ptr<Node> {
		let head = self.base.next.get();

		/*
		 * Safety: the node is in our list
		 *
		 * calling head().unlink() should optimize away the linked check,
		 * also aborts in debug if the condition isn't satisfied
		 */
		unsafe { assume(ptr!(head=>linked())) };

		head
	}

	pub fn tail(&self) -> Ptr<Node> {
		let tail = self.base.next.get();

		/*
		 * Safety: the node is in our list
		 *
		 * calling tail().unlink() should optimize away the linked check
		 * also aborts in debug if the condition isn't satisfied
		 */
		unsafe { assume(ptr!(tail=>linked())) };

		tail
	}

	/// # Safety
	/// same as remove, and node must not be base if the list isn't empty
	unsafe fn pop_edge(&self, node: Ptr<Node>) -> Option<Ptr<Node>> {
		if !self.is_empty() {
			/* Safety: guaranteed by caller */
			unsafe { self.remove(node.as_ref()) };

			Some(node)
		} else {
			None
		}
	}

	pub fn pop_front(&self) -> Option<Ptr<Node>> {
		/* Safety: we are removing a node from our list */
		unsafe { self.pop_edge(self.head()) }
	}

	pub fn pop_back(&self) -> Option<Ptr<Node>> {
		/* Safety: we are removing a node from our list */
		unsafe { self.pop_edge(self.tail()) }
	}

	/// # Safety
	/// This list must be pinned, Node must be pinned and live while it's in the
	/// list, and not already be in a list
	///
	/// The reference to the node is considered borrowed until it is unlinked
	/// from the list any usages of mutable references to the node is considered
	/// UB
	pub unsafe fn append(&self, node: &Node) {
		/* Safety: node is only ever added to one list at a time, so we have
		 * exclusive access to the node. The list now has the right to create a
		 * mutable borrow to the inner, so we cannot expect a &mut Node in the
		 * function signature. Base and prev must live as long as they are linked */
		unsafe { node.set_ptrs(self.base.prev.get().as_ref(), &self.base) };
	}

	/// # Safety
	/// This list must be pinned, Node must be pinned and linked to this list
	pub unsafe fn remove(&self, node: &Node) {
		/* Safety: guaranteed by caller */
		unsafe { node.unlink_unchecked() };
	}

	/// # Safety
	/// The new list must be pinned, empty, and live as long as it has nodes
	pub unsafe fn move_elements(&self, other: &Self) {
		if self.is_empty() {
			return;
		}

		/* Safety: guaranteed by caller */
		unsafe { assert_unsafe_precondition!(other.is_empty()) };

		let (prev, next) = (self.base.prev.get(), self.base.next.get());

		/* Safety: all nodes must live as long as they are linked */
		unsafe { other.base.set_ptrs(prev.as_ref(), next.as_ref()) };

		/* Safety: we're now empty */
		unsafe { self.base.set_circular() };
	}
}

impl Pin for LinkedList {
	unsafe fn pin(&mut self) {
		/* Safety: we are being pinned */
		unsafe { self.base.set_circular() };
	}
}

impl Default for LinkedList {
	fn default() -> Self {
		Self::new()
	}
}
