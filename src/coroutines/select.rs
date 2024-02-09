use std::{mem::replace, result};

use super::*;

pub enum Select<O1, O2> {
	First(O1, Option<O2>),
	Second(O2, Option<O1>)
}

impl<O1, O2> Select<O1, O2> {
	pub fn first(self) -> Option<O1> {
		match self {
			Select::First(result, _) => Some(result),
			Select::Second(..) => None
		}
	}

	pub fn second(self) -> Option<O2> {
		match self {
			Select::First(..) => None,
			Select::Second(result, _) => Some(result)
		}
	}
}

impl<O1, O2, E> Select<result::Result<O1, E>, result::Result<O2, E>> {
	/// Flatten the `Select`, returning the first error it encounters
	pub fn flatten(self) -> result::Result<Select<O1, O2>, E> {
		Ok(match self {
			Select::First(a, b) => Select::First(
				a?,
				match b {
					None => None,
					Some(b) => Some(b?)
				}
			),

			Select::Second(a, b) => Select::Second(
				a?,
				match b {
					None => None,
					Some(b) => Some(b?)
				}
			)
		})
	}
}

impl<O1, O2> Select<Option<O1>, Option<O2>> {
	/// Flatten the `Select`, returning none if there are any
	pub fn flatten(self) -> Option<Select<O1, O2>> {
		Some(match self {
			Select::First(a, b) => Select::First(
				a?,
				match b {
					None => None,
					Some(b) => Some(b?)
				}
			),

			Select::Second(a, b) => Select::Second(
				a?,
				match b {
					None => None,
					Some(b) => Some(b?)
				}
			)
		})
	}
}

struct SelectData<T1: SyncTask, T2: SyncTask> {
	handle_1: TaskHandle<T1>,
	handle_2: TaskHandle<T2>,
	request: ReqPtr<Select<T1::Output, T2::Output>>,
	sync_done: UnsafeCell<bool>
}

impl<T1: SyncTask, T2: SyncTask> SelectData<T1, T2> {
	unsafe fn complete(&mut self, is_first: bool) {
		let sync_done = self.sync_done.as_mut();

		if *sync_done {
			*sync_done = false;

			return;
		}

		/*
		 * Safety: cannot access `self` once a cancel or a complete is called,
		 * as it may be freed by the callee
		 */
		if !self.handle_1.done() || !self.handle_2.done() {
			unsafe {
				if is_first {
					self.handle_2.try_cancel()
				} else {
					self.handle_1.try_cancel()
				}
			};
		} else {
			/* reverse order, because this is the last task to complete */
			let result = if is_first {
				Select::Second(self.handle_2.take_result(), self.handle_1.result.take())
			} else {
				Select::First(self.handle_1.take_result(), self.handle_2.result.take())
			};

			Request::complete(self.request, result);
		}
	}

	unsafe fn complete_1(_: ReqPtr<T1::Output>, arg: Ptr<()>, value: T1::Output) {
		let this = arg.cast::<Self>().cast_mut().as_mut();

		this.handle_1.complete(value);
		this.complete(true);
	}

	unsafe fn complete_2(_: ReqPtr<T2::Output>, arg: Ptr<()>, value: T2::Output) {
		let this = arg.cast::<Self>().cast_mut().as_mut();

		this.handle_2.complete(value);
		this.complete(false);
	}

	fn new(task_1: T1, task_2: T2) -> Self {
		/* request args are assigned once pinned */
		Self {
			handle_1: TaskHandle::new(task_1, Self::complete_1),
			handle_2: TaskHandle::new(task_2, Self::complete_2),
			request: Ptr::null(),
			sync_done: UnsafeCell::new(false)
		}
	}

	#[future]
	unsafe fn select(&mut self) -> Select<T1::Output, T2::Output> {
		fn cancel(mut self: MutPtr<Self>) -> Result<()> {
			let (cancel_1, cancel_2) = unsafe {
				/* must prevent cancel 1 from calling cancel 2 in callback, as we need to
				 * access it */
				let cancel = (self.handle_1.cancel.take(), self.handle_2.cancel.take());

				(
					cancel.0.map(|cancel| cancel.run()),
					cancel.1.map(|cancel| cancel.run())
				)
			};

			if let Some(Err(result)) = cancel_1 {
				return Err(result);
			}

			if let Some(Err(result)) = cancel_2 {
				return Err(result);
			}

			Ok(())
		}

		unsafe { self.handle_1.run() };

		if self.handle_1.done() {
			return Progress::Done(Select::First(self.handle_1.take_result(), None));
		}

		unsafe { self.handle_2.run() };

		if self.handle_2.done() {
			*self.sync_done.as_mut() = true;

			let _ = unsafe { self.handle_1.cancel() };

			if !replace(self.sync_done.as_mut(), false) {
				return Progress::Done(Select::Second(
					self.handle_2.take_result(),
					self.handle_1.result.take()
				));
			}
		}

		self.request = request;

		Progress::Pending(cancel(self.into(), request))
	}
}

unsafe impl<T1: SyncTask, T2: SyncTask> Pin for SelectData<T1, T2> {
	unsafe fn pin(&mut self) {
		let arg = self.into();

		self.handle_1.set_arg(arg);
		self.handle_2.set_arg(arg);
	}
}

#[asynchronous]
pub async unsafe fn select_sync_task<T1: SyncTask, T2: SyncTask>(
	task_1: T1, task_2: T2
) -> Select<T1::Output, T2::Output> {
	let mut data = SelectData::new(task_1, task_2);

	block_on(data.pin_local().select()).await
}

/// Races two tasks A and B and waits
/// for one of them to finish and cancelling the other
///
/// Returns `Select::First` if the first task completed first
/// or `Select::Second` if the second task completed first
///
/// Because a task may not be cancelled in time, the second parameter
/// in `Select` may contain the result from the cancelled task
#[asynchronous]
pub async unsafe fn select<R: Environment, T1: Task, T2: Task>(
	runtime: Ptr<R>, task_1: T1, task_2: T2
) -> Select<T1::Output, T2::Output> {
	select_sync_task(
		spawn_future_with_env(runtime, task_1),
		spawn_future_with_env(runtime, task_2)
	)
	.await
}
