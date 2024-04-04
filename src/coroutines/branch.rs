#![allow(
	clippy::multiple_unsafe_ops_per_block,
	clippy::module_name_repetitions,
	unreachable_pub
)]

use std::{cell::Cell, mem::replace};

use super::*;
use crate::runtime::PanickingResult;

/// # Safety
/// the future must not be complete
unsafe fn run_cancel<C>(cancel: C) -> PanickingResult<Result<()>>
where
	C: Cancel
{
	let result = catch_unwind(AssertUnwindSafe(|| {
		/* Safety: contract upheld by caller */
		unsafe { cancel.run() }
	}));

	match &result {
		Ok(Err(err)) => debug!("Cancel was not successful: {:?}", err),
		Err(_) => warn!("Cancel panicked"),
		_ => ()
	}

	result
}

enum State<F: Future> {
	Ready(F),
	Pending(F::Cancel),
	Done(F::Output),
	Empty
}

struct FutureHandle<F: Future> {
	state: UnsafeCell<State<F>>,
	request: Request<F::Output>
}

impl<F: Future> FutureHandle<F> {
	/// # Safety
	/// See `Request::new`
	unsafe fn new(future: F, callback: Complete<F::Output>) -> Self {
		Self {
			state: UnsafeCell::new(State::Ready(future)),

			/* Safety: guaranteed by caller */
			request: unsafe { Request::new(Ptr::null(), callback) }
		}
	}

	/// # Safety
	/// See `Future::run`
	/// Must not call twice
	/// Must call `FutureHandle::complete` when the future finishes
	/// Request arg must be set
	unsafe fn run(&mut self) -> PanickingResult<Option<&mut F::Output>> {
		let state = self.state.get_mut();

		let future = match replace(state, State::Empty) {
			State::Ready(future) => future,

			/* Safety: guaranteed by caller */
			_ => unsafe { unreachable_unchecked!("`FutureHandle::run` called twice") }
		};

		let progress = catch_unwind(AssertUnwindSafe(|| {
			/* Safety: guaranteed by caller */
			unsafe { future.run(ptr!(&self.request)) }
		}))?;

		Ok(match progress {
			Progress::Pending(cancel) => {
				*state = State::Pending(cancel);

				None
			}

			Progress::Done(value) => {
				/* Safety: the future completed synchronously */
				Some(unsafe { self.complete(value) })
			}
		})
	}

	fn done(&mut self) -> bool {
		matches!(self.state.get_mut(), State::Done(_))
	}

	/// # Safety
	/// must only call once, when the future is finished
	unsafe fn complete(&mut self, result: F::Output) -> &mut F::Output {
		if self.done() {
			/* Safety: guaranteed by caller */
			unsafe { unreachable_unchecked!("`FutureHandle::complete` called twice") };
		}

		let state = self.state.get_mut();

		*state = State::Done(result);

		match state {
			State::Done(value) => value,
			/* Safety: we just stored a Done */
			_ => unsafe { unreachable_unchecked() }
		}
	}

	/// # Safety
	/// the future must have completed, and `FutureHandle::complete` was called
	/// with the result
	unsafe fn result(&mut self) -> F::Output {
		match replace(self.state.get_mut(), State::Empty) {
			State::Done(value) => value,

			/* abort here (on debug) because this is most definitely fatal, and is usually called
			 * within Request::complete
			 *
			 * Safety: guaranteed by caller
			 */
			_ => unsafe {
				unreachable_unchecked!(
					"Fatal error: called `FutureHandle::result` on an in progress future"
				)
			}
		}
	}

	fn take_cancel(&mut self) -> Option<F::Cancel> {
		let state = self.state.get_mut();

		if !matches!(state, State::Pending(_)) {
			return None;
		}

		match replace(state, State::Empty) {
			State::Pending(cancel) => Some(cancel),

			/* Safety: just checked */
			_ => unsafe { unreachable_unchecked() }
		}
	}

	/// # Safety
	/// `this` must be a valid pointer
	/// there must be no references to Self
	/// `this` may become dangling after the function call
	unsafe fn try_cancel_catch_unwind(this: MutPtr<Self>) -> Option<PanickingResult<Result<()>>> {
		/* Safety: guaranteed by caller */
		let cancel = unsafe { ptr!(this=>take_cancel()) };

		cancel.map(|cancel| {
			/* Safety: cancel is None if future isn't running */
			unsafe { run_cancel(cancel) }
		})
	}

	/// # Safety
	/// the future must be in progress
	/// See `try_cancel_catch_unwind`
	unsafe fn cancel_catch_unwind(this: MutPtr<Self>) -> PanickingResult<Result<()>> {
		/* Safety: guaranteed by caller */
		match unsafe { Self::try_cancel_catch_unwind(this) } {
			Some(result) => result,
			/* Safety: guaranteed by caller */
			None => unsafe { unreachable_unchecked!("`FutureHandle::cancel` called twice") }
		}
	}

	fn set_arg(&mut self, arg: Ptr<()>) {
		self.request.set_arg(arg);
	}
}

pub struct BranchOutput<O1, O2>(pub bool, pub Option<O1>, pub Option<O2>);

pub struct Branch<F1: Future, F2: Future, Cancel> {
	handles: (FutureHandle<F1>, FutureHandle<F2>),
	request: ReqPtr<BranchOutput<F1::Output, F2::Output>>,
	should_cancel: Cancel,
	sync_done: Cell<bool>
}

#[future]
impl<
		F1: Future,
		F2: Future,
		C1: Fn(PanickingResult<&F1::Output>) -> bool,
		C2: Fn(PanickingResult<&F2::Output>) -> bool
	> Branch<F1, F2, (C1, C2)>
{
	unsafe fn complete_single(this: MutPtr<Self>, is_first: bool, should_cancel: bool) {
		/* Safety: we have mutable access here */
		let this = unsafe { this.as_mut() };

		if this.sync_done.replace(false) {
			return;
		}

		if this.handles.0.done() && this.handles.1.done() {
			/* Safety: both futures finished */
			let result = unsafe {
				BranchOutput(
					/* reverse order, because this is the last future to complete */
					!is_first,
					Some(this.handles.0.result()),
					Some(this.handles.1.result())
				)
			};

			/*
			 * Safety: complete the future. we must not access `self` once a cancel or a
			 * complete is called, as we may be freed by the callee
			 *
			 * `this` is transitioned to a Disabled state after this call
			 * which is okay, because it's not a protected tag
			 */
			unsafe { Request::complete(this.request, result) };

			return;
		}

		if !should_cancel {
			return;
		}

		/* Safety: if both aren't complete, then the other must be running
		 *
		 * `this` is transitioned to a Disabled state after this call
		 * which is okay, because it's not a protected tag
		 */
		unsafe {
			/* we can't do much if the cancel panics */
			let _ = if is_first {
				FutureHandle::cancel_catch_unwind(ptr!(&mut this.handles.1))
			} else {
				FutureHandle::cancel_catch_unwind(ptr!(&mut this.handles.0))
			};
		}
	}

	unsafe fn complete_first(_: ReqPtr<F1::Output>, arg: Ptr<()>, value: F1::Output) {
		let this = arg.cast::<Self>().cast_mut();

		/* Safety: the future has completed */
		let result = unsafe { ptr!(this=>handles.0.complete(value)) };

		/* Safety: guaranteed by Future's contract */
		let should_cancel = unsafe { ptr!(this=>should_cancel.0(Ok(result))) };

		/* Safety: the future has completed */
		unsafe { Self::complete_single(this, true, should_cancel) };
	}

	unsafe fn complete_second(_: ReqPtr<F2::Output>, arg: Ptr<()>, value: F2::Output) {
		let this = arg.cast::<Self>().cast_mut();

		/* Safety: the future has completed */
		let result = unsafe { ptr!(this=>handles.1.complete(value)) };

		/* Safety: guaranteed by Future's contract */
		let should_cancel = unsafe { ptr!(this=>should_cancel.1(Ok(result))) };

		/* Safety: the future has completed */
		unsafe { Self::complete_single(this, false, should_cancel) };
	}

	pub fn new(future_1: F1, future_2: F2, should_cancel: (C1, C2)) -> Self {
		/* Safety: complete does not panic */
		unsafe {
			/* request args are assigned once pinned */
			Self {
				handles: (
					FutureHandle::new(future_1, Self::complete_first),
					FutureHandle::new(future_2, Self::complete_second)
				),
				request: Ptr::null(),
				should_cancel,
				sync_done: Cell::new(false)
			}
		}
	}

	unsafe fn cancel_all(this: MutPtr<Self>) -> Result<()> {
		let cancels = {
			/* Safety: guaranteed by future's contract */
			let this = unsafe { this.as_mut() };

			/* it's insufficient to cancel directly
			 * future 1: pending
			 * future 2: done
			 *
			 * cancel future 1: completes synchronously, completes the branch
			 * cancel future 2: use-after-free
			 */
			let cancels = (this.handles.0.take_cancel(), this.handles.1.take_cancel());

			/* must prevent cancel 1 from trying to call cancel 2 in callback, which is a
			 * None */
			if cancels.0.is_some() && cancels.1.is_some() {
				this.sync_done.set(true);
			}

			cancels
		};

		/* Safety: cancel is None if one of the futures already completed. if both
		 * completed, we wouldn't be here because caller must uphold Future's
		 * contract
		 */
		let cancel = unsafe {
			[
				cancels.0.map(|cancel| run_cancel(cancel)),
				cancels.1.map(|cancel| run_cancel(cancel))
			]
		};

		let mut result = Ok(());

		for cancel in cancel {
			let Some(cancel_result) = runtime::join(cancel.transpose()) else {
				continue;
			};

			if result.is_ok() {
				result = cancel_result;
			}
		}

		result
	}

	/// # Safety
	/// see `Future::run`
	/// self must be pinned
	/// `this` must be a valid pointer
	#[future]
	pub unsafe fn run(this_ptr: MutPtr<Self>) -> BranchOutput<F1::Output, F2::Output> {
		#[cancel]
		fn cancel(self: MutPtr<Self>) -> Result<()> {
			/* Safety: caller must uphold Future's contract */
			unsafe { Self::cancel_all(self) }
		}

		/* Safety: guaranteed by caller */
		let this = unsafe { this_ptr.as_mut() };

		this.request = request;

		/* Safety: caller must uphold Future's contract */
		if let Some(result) = runtime::join(unsafe { this.handles.0.run() }) {
			if this.should_cancel.0(Ok(result)) {
				/* Safety: future completed */
				let result = unsafe { this.handles.0.result() };

				return Progress::Done(BranchOutput(true, Some(result), None));
			}
		}

		#[allow(clippy::never_loop)]
		loop {
			/* Safety: caller must uphold Future's contract */
			let Some(result) = unsafe { this.handles.1.run() }.transpose() else {
				break;
			};

			let done = this.handles.0.done();

			if done {
				runtime::join(result);
			} else if this.should_cancel.1(result.map(|output| &*output)) {
				this.sync_done.set(true);

				/* Safety: reborrow may occur in cancel */
				unsafe { this.pin() };

				/* Safety: future is in progress */
				let _ = unsafe { FutureHandle::cancel_catch_unwind(ptr!(&mut this.handles.0)) };

				if this.sync_done.replace(false) {
					break;
				}
			}

			/* Safety: both futures completed */
			let result = unsafe {
				BranchOutput(
					done,
					Some(this.handles.0.result()),
					Some(this.handles.1.result())
				)
			};

			return Progress::Done(result);
		}

		Progress::Pending(cancel(this_ptr, request))
	}
}

impl<F1: Future, F2: Future, Cancel> Pin for Branch<F1, F2, Cancel> {
	unsafe fn pin(&mut self) {
		let arg = ptr!(&*self);

		self.handles.0.set_arg(arg.cast());
		self.handles.1.set_arg(arg.cast());
	}
}

#[asynchronous]
pub async fn branch<F1, F2, C1, C2>(
	future_1: F1, future_2: F2, should_cancel: (C1, C2)
) -> BranchOutput<F1::Output, F2::Output>
where
	F1: Future,
	F2: Future,
	C1: Fn(PanickingResult<&F1::Output>) -> bool,
	C2: Fn(PanickingResult<&F2::Output>) -> bool
{
	let mut branch = Branch::new(future_1, future_2, should_cancel);

	/* Safety: branch is pinned. we are blocked until the future completes */
	block_on(unsafe { Branch::run(ptr!(&mut *branch.pin_local())) }).await
}
