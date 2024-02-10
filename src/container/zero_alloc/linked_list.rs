use super::*;

struct NodeInner {
	prev: MutPtr<NodeInner>,
	next: MutPtr<NodeInner>
}

impl NodeInner {
	fn new() -> Self {
		Self { prev: MutPtr::null(), next: MutPtr::null() }
	}

	fn linked(&self) -> bool {
		!self.prev.is_null()
	}

	unsafe fn set_next(&mut self, next: &mut Self) {
		self.next = next.into();

		next.prev = self.into();
	}

	unsafe fn set_prev(&mut self, prev: &mut Self) {
		self.prev = prev.into();

		prev.next = self.into();
	}

	unsafe fn set_ptrs(&mut self, prev: &mut Self, next: &mut Self) {
		self.set_prev(prev);
		self.set_next(next);
	}

	unsafe fn unlink(&mut self) {
		self.prev.as_mut().next = self.next;
		self.next.as_mut().prev = self.prev;
		*self = Self::new();
	}
}

#[repr(transparent)]
pub struct Node {
	inner: UnsafeCell<NodeInner>
}

impl Node {
	pub fn new() -> Self {
		Self { inner: UnsafeCell::new(NodeInner::new()) }
	}

	pub fn linked(&self) -> bool {
		self.inner.as_ref().linked()
	}

	pub unsafe fn unlink_unchecked(&self) {
		self.inner.as_mut().unlink();
	}

	pub unsafe fn unlink(&self) {
		assert!(self.linked());

		self.unlink_unchecked();
	}
}

pub struct LinkedList {
	base: Node
}

impl LinkedList {
	unsafe fn pin_base(&self) {
		let base = self.base.inner.as_mut();

		base.prev = base.into();
		base.next = base.into();
	}

	pub fn new() -> Self {
		Self { base: Node::new() }
	}

	pub fn empty(&self) -> bool {
		let base = self.base.inner.get();

		base == unsafe { base.as_ref().next }
	}

	pub fn head(&self) -> Ptr<Node> {
		let base = self.base.inner.get();

		unsafe { base.as_ref().next }.cast_const().cast()
	}

	pub fn tail(&self) -> Ptr<Node> {
		let base = self.base.inner.get();

		unsafe { base.as_ref().prev }.cast_const().cast()
	}

	pub unsafe fn append(&self, node: &Node) {
		let base = self.base.inner.as_mut();
		let node = node.inner.as_mut();

		node.set_ptrs(base.prev.as_mut(), base);
	}

	pub unsafe fn remove(&self, node: &Node) {
		node.unlink();
	}

	pub unsafe fn move_elements(&self, other: &Self) {
		if self.empty() {
			return;
		}

		let base = self.base.inner.as_mut();
		let other = other.base.inner.as_mut();

		other.set_ptrs(base.prev.as_mut(), base.next.as_mut());

		self.pin_base();
	}
}

unsafe impl Pin for LinkedList {
	unsafe fn pin(&mut self) {
		self.pin_base();
	}
}
