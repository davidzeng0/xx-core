#![allow(
	clippy::multiple_unsafe_ops_per_block,
	clippy::module_name_repetitions,
	unreachable_pub
)]

use std::{cell::Cell, mem::replace};

use super::*;

pub type PanickingResult<T> = std::thread::Result<T>;

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
	/// The future must not have been started
	/// Must call `FutureHandle::complete` when the future finishes, if it
	/// doesn't complete immediately
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
			unsafe { future.run(Ptr::from(&self.request)) }
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

	fn done(&self) -> bool {
		/* Safety: exclusive unsafe cell access */
		matches!(unsafe { self.state.as_ref() }, State::Done(_))
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

	fn take_cancel(&self) -> Option<F::Cancel> {
		/* Safety: exclusive unsafe cell access */
		let state = unsafe { self.state.as_mut() };

		if matches!(state, State::Pending(_)) {
			match replace(state, State::Empty) {
				State::Pending(cancel) => Some(cancel),

				/* Safety: just checked */
				_ => unsafe { unreachable_unchecked() }
			}
		} else {
			None
		}
	}

	fn try_cancel_catch_unwind(&self) -> Option<PanickingResult<Result<()>>> {
		self.take_cancel().map(|cancel| {
			/* Safety: cancel is None if future isn't running */
			unsafe { run_cancel(cancel) }
		})
	}

	/// # Safety
	/// the future must be in progress
	unsafe fn cancel_catch_unwind(&self) -> PanickingResult<Result<()>> {
		match self.try_cancel_catch_unwind() {
			Some(result) => result,
			/* Safety: guaranteed by caller */
			None => unsafe { unreachable_unchecked!("`FutureHandle::cancel` called twice") }
		}
	}

	fn set_arg<A>(&mut self, arg: Ptr<A>) {
		self.request.set_arg(arg.as_unit());
	}
}

pub struct BranchOutput<O1, O2>(pub bool, pub Option<O1>, pub Option<O2>);

pub struct Branch<F1: Future, F2: Future, Cancel> {
	handles: (FutureHandle<F1>, FutureHandle<F2>),
	request: ReqPtr<BranchOutput<F1::Output, F2::Output>>,
	should_cancel: Cancel,
	sync_done: Cell<bool>
}

impl<
		F1: Future,
		F2: Future,
		C1: Fn(PanickingResult<&F1::Output>) -> bool,
		C2: Fn(PanickingResult<&F2::Output>) -> bool
	> Branch<F1, F2, (C1, C2)>
{
	unsafe fn complete_single(&mut self, is_first: bool, should_cancel: bool) {
		if self.sync_done.replace(false) {
			return;
		}

		if self.handles.0.done() && self.handles.1.done() {
			/* Safety: both futures finished */
			let result = unsafe {
				BranchOutput(
					/* reverse order, because this is the last future to complete */
					!is_first,
					Some(self.handles.0.result()),
					Some(self.handles.1.result())
				)
			};

			/*
			 * Safety: complete the future. we must not access `self` once a cancel or a
			 * complete is called, as we may be freed by the callee
			 */
			unsafe { Request::complete(self.request, result) };

			return;
		}

		if !should_cancel {
			return;
		}

		/* Safety: if both aren't complete, then the other must be running */
		unsafe {
			/* we can't do much if the cancel panics */
			let _ = if is_first {
				self.handles.1.cancel_catch_unwind()
			} else {
				self.handles.0.cancel_catch_unwind()
			};
		}
	}

	unsafe fn complete_first(_: ReqPtr<F1::Output>, arg: Ptr<()>, value: F1::Output) {
		/* Safety: guaranteed by Future's contract */
		let this = unsafe { arg.cast::<Self>().cast_mut().as_mut() };
		let should_cancel = this.should_cancel.0(Ok(&value));

		/* Safety: the future has completed */
		unsafe {
			this.handles.0.complete(value);
			this.complete_single(true, should_cancel);
		}
	}

	unsafe fn complete_second(_: ReqPtr<F2::Output>, arg: Ptr<()>, value: F2::Output) {
		/* Safety: guaranteed by Future's contract */
		let this = unsafe { arg.cast::<Self>().cast_mut().as_mut() };
		let should_cancel = this.should_cancel.1(Ok(&value));

		/* Safety: the future has completed */
		unsafe {
			this.handles.1.complete(value);
			this.complete_single(false, should_cancel);
		}
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

	fn cancel_all(&self) -> Result<()> {
		/* must prevent cancel 1 from calling cancel 2 in callback, as we need to
		 * access it */
		self.sync_done.set(true);

		/* cancel is None if one of the futures already completed. if both
		 * completed, we wouldn't be here because caller must uphold Future's
		 * contract */
		let cancel = [
			self.handles.0.try_cancel_catch_unwind(),
			self.handles.1.try_cancel_catch_unwind()
		];

		let mut result = Ok(());

		for cancel in cancel {
			let Some(cancel_result) = unwrap_panic!(cancel.transpose()) else {
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
	#[future]
	pub unsafe fn run(&mut self) -> BranchOutput<F1::Output, F2::Output> {
		#[cancel]
		fn cancel(self: Ptr<Self>) -> Result<()> {
			/* Safety: caller must uphold Future's contract */
			unsafe { self.as_ref().cancel_all() }
		}

		self.request = request;

		/* Safety: caller must uphold Future's contract */
		if let Some(result) = unwrap_panic!(unsafe { self.handles.0.run() }) {
			if self.should_cancel.0(Ok(result)) {
				/* Safety: future completed */
				let result = unsafe { self.handles.0.result() };

				return Progress::Done(BranchOutput(true, Some(result), None));
			}
		}

		#[allow(clippy::never_loop)]
		loop {
			/* Safety: caller must uphold Future's contract */
			let Some(result) = unsafe { self.handles.1.run() }.transpose() else {
				break;
			};

			let done = self.handles.0.done();

			if done {
				unwrap_panic!(result);
			} else if self.should_cancel.1(result.map(|output| &*output)) {
				self.sync_done.set(true);

				/* Safety: let compiler know about our (possible) temporary reborrow */
				unsafe { self.pin() };

				/* Safety: future is in progress */
				let _ = unsafe { self.handles.0.cancel_catch_unwind() };

				if self.sync_done.replace(false) {
					break;
				}
			}

			/* Safety: both futures completed */
			let result = unsafe {
				BranchOutput(
					done,
					Some(self.handles.0.result()),
					Some(self.handles.1.result())
				)
			};

			return Progress::Done(result);
		}

		/* we no longer have mutable access */
		Progress::Pending(cancel(MutPtr::from(self).cast_const(), request))
	}
}

impl<F1: Future, F2: Future, Cancel> Pin for Branch<F1, F2, Cancel> {
	unsafe fn pin(&mut self) {
		let arg = MutPtr::from(&mut *self).cast_const();

		self.handles.0.set_arg(arg);
		self.handles.1.set_arg(arg);
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
	block_on(unsafe { branch.pin_local().run() }).await
}
