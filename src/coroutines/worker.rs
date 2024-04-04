#![allow(clippy::multiple_unsafe_ops_per_block)]

use std::cell::Cell;

use super::*;

/// A worker thread capable of running async operations via fibers
pub struct Worker {
	executor: Ptr<Executor>,
	caller: Cell<Ptr<Worker>>,
	fiber: UnsafeCell<Fiber>
}

impl Worker {
	/// The worker for the main thread, which does not need
	/// an extra stack allocation, because it's allocated for us
	#[must_use]
	pub fn main() -> Self {
		/* Safety: this is the main fiber */
		unsafe { Self::from_fiber(Ptr::null(), Fiber::main()) }
	}

	/// Creates a new worker with the starting point `start`
	///
	/// # Safety
	/// executor must be valid for the duration of the worker
	#[must_use]
	pub unsafe fn new(executor: Ptr<Executor>, start: Start) -> Self {
		/* Safety: contract is upheld by caller */
		unsafe { Self::from_fiber(executor, Fiber::new_with_start(start)) }
	}

	/// # Safety
	/// executor must be valid for the duration of the worker, unless its the
	/// main fiber
	#[must_use]
	pub unsafe fn from_fiber(executor: Ptr<Executor>, fiber: Fiber) -> Self {
		Self {
			executor,

			/* from is initialized later */
			caller: Cell::new(Ptr::null()),
			fiber: UnsafeCell::new(fiber)
		}
	}

	/// The worker that `self` will resume to when suspending
	pub(super) fn caller(&self) -> Ptr<Self> {
		self.caller.get()
	}

	pub(super) unsafe fn suspend_to(&self, to: Ptr<Self>) {
		self.caller.set(to);
	}

	pub(super) fn fiber(&self) -> MutPtr<Fiber> {
		self.fiber.get()
	}

	pub(super) fn into_inner(self) -> Fiber {
		self.fiber.into_inner()
	}

	/// # Safety
	/// see `Executor::resume`
	pub(super) unsafe fn resume(&self) {
		/* Safety: guaranteed by caller */
		unsafe { ptr!(self.executor=>resume(ptr!(self))) };
	}

	/// # Safety
	/// see `Executor::suspend`
	pub(super) unsafe fn suspend(&self) {
		/* Safety: guaranteed by caller */
		unsafe { ptr!(self.executor=>suspend(ptr!(self))) };
	}

	/// # Safety
	/// see `Executor::exit`
	pub(super) unsafe fn exit(self) {
		/* Safety: guaranteed by caller */
		unsafe { ptr!(self.executor=>exit(self)) };
	}
}

impl Pin for Worker {
	unsafe fn pin(&mut self) {
		/* Safety: we are being pinned */
		unsafe { ptr!(self.executor=>worker_pinned(ptr!(&*self))) };
	}
}
