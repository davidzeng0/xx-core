use std::result;

use super::*;
use crate::warn;

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
	task_1: Option<T1>,
	req_1: Request<T1::Output>,
	cancel_1: Option<T1::Cancel>,
	result_1: Option<T1::Output>,

	task_2: Option<T2>,
	req_2: Request<T2::Output>,
	cancel_2: Option<T2::Cancel>,
	result_2: Option<T2::Output>,

	request: RequestPtr<Select<T1::Output, T2::Output>>,
	sync_done: bool
}

impl<T1: SyncTask, T2: SyncTask> SelectData<T1, T2> {
	fn complete(&mut self, is_first: bool) {
		if self.sync_done {
			self.sync_done = false;

			return;
		}

		/*
		 * Safety: cannot access `self` once a cancel or a complete is called,
		 * as it may be freed by the callee
		 */
		if self.result_1.is_none() || self.result_2.is_none() {
			let result = unsafe {
				if is_first {
					self.cancel_2.take().map(|cancel| cancel.run())
				} else {
					self.cancel_1.take().map(|cancel| cancel.run())
				}
			};

			if let Some(result) = result {
				if result.is_err() {
					warn!("Cancel returned an {:?}", result);
				}
			}
		} else {
			/* reverse order, because this is the last task to complete */
			let result = if is_first {
				Select::Second(self.result_2.take().unwrap(), self.result_1.take())
			} else {
				Select::First(self.result_1.take().unwrap(), self.result_2.take())
			};

			Request::complete(self.request, result);
		}
	}

	fn complete_1(_: RequestPtr<T1::Output>, arg: Ptr<()>, value: T1::Output) {
		let this = arg.cast::<Self>().make_mut().as_mut();

		this.result_1 = Some(value);
		this.complete(true);
	}

	fn complete_2(_: RequestPtr<T2::Output>, arg: Ptr<()>, value: T2::Output) {
		let this = arg.cast::<Self>().make_mut().as_mut();

		this.result_2 = Some(value);
		this.complete(false);
	}

	fn new(task_1: T1, task_2: T2) -> Self {
		unsafe {
			/* request args are assigned once pinned */
			Self {
				task_1: Some(task_1),
				req_1: Request::new(Ptr::null(), Self::complete_1),
				cancel_1: None,
				result_1: None,

				task_2: Some(task_2),
				req_2: Request::new(Ptr::null(), Self::complete_2),
				cancel_2: None,
				result_2: None,

				request: Ptr::null(),
				sync_done: false
			}
		}
	}

	#[sync_task]
	fn select(&mut self) -> Select<T1::Output, T2::Output> {
		fn cancel(self: &mut Self) -> Result<()> {
			let (cancel_1, cancel_2) = unsafe {
				/* must prevent cancel 1 from calling cancel 2 in callback */
				let cancel = (self.cancel_1.take(), self.cancel_2.take());

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

		unsafe {
			match self.task_1.take().unwrap().run(Ptr::from(&self.req_1)) {
				Progress::Pending(cancel) => self.cancel_1 = Some(cancel),
				Progress::Done(value) => return Progress::Done(Select::First(value, None))
			}

			match self.task_2.take().unwrap().run(Ptr::from(&self.req_2)) {
				Progress::Pending(cancel) => self.cancel_2 = Some(cancel),
				Progress::Done(value) => {
					self.result_2 = Some(value);
					self.sync_done = true;

					let result = self.cancel_1.take().unwrap().run();

					if result.is_err() {
						warn!("Cancel returned an {:?}", result);
					}

					if !self.sync_done {
						return Progress::Done(Select::Second(
							self.result_2.take().unwrap(),
							self.result_1.take()
						));
					}

					self.sync_done = false;
				}
			}

			self.request = request;

			return Progress::Pending(cancel(self, request));
		}
	}
}

impl<T1: SyncTask, T2: SyncTask> Global for SelectData<T1, T2> {
	unsafe fn pinned(&mut self) {
		let mut this = MutPtr::from(self);
		let arg = this.as_unit().into();

		this.req_1.set_arg(arg);
		this.req_2.set_arg(arg);
	}
}

#[async_fn]
pub async unsafe fn select_sync<T1: SyncTask, T2: SyncTask>(
	task_1: T1, task_2: T2
) -> Select<T1::Output, T2::Output> {
	let data = SelectData::new(task_1, task_2);

	pin_local_mut!(data);
	block_on(data.select()).await
}

/// Races two tasks A and B and waits
/// for one of them to finish and cancelling the other
///
/// Returns `Select::First` if the first task completed first
/// or `Select::Second` if the second task completed first
///
/// Because a task may not be cancelled in time, the second parameter
/// in `Select` may contain the result from the cancelled task
#[async_fn]
pub async unsafe fn select<R: PerContextRuntime, T1: Task, T2: Task>(
	runtime: Handle<R>, task_1: T1, task_2: T2
) -> Select<T1::Output, T2::Output> {
	select_sync(
		spawn_sync_with_runtime(runtime, task_1),
		spawn_sync_with_runtime(runtime, task_2)
	)
	.await
}
