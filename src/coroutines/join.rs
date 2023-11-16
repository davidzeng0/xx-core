use std::result;

use super::*;

pub struct Join<O1, O2>(pub O1, pub O2);

impl<O1, O2, E> Join<result::Result<O1, E>, result::Result<O2, E>> {
	/// Flatten the `Join``, returning the first error it encounters
	pub fn flatten(self) -> result::Result<Join<O1, O2>, E> {
		Ok(Join(self.0?, self.1?))
	}
}

impl<O1, O2> Join<Option<O1>, Option<O2>> {
	/// Flatten the `Join``, returning none if there are any
	pub fn flatten(self) -> Option<Join<O1, O2>> {
		Some(Join(self.0?, self.1?))
	}
}

struct JoinData<T1: SyncTask, T2: SyncTask> {
	task_1: Option<T1>,
	req_1: Request<T1::Output>,
	cancel_1: Option<T1::Cancel>,
	result_1: Option<T1::Output>,

	task_2: Option<T2>,
	req_2: Request<T2::Output>,
	cancel_2: Option<T2::Cancel>,
	result_2: Option<T2::Output>,

	request: RequestPtr<Join<T1::Output, T2::Output>>
}

impl<T1: SyncTask, T2: SyncTask> JoinData<T1, T2> {
	fn complete_1(_: RequestPtr<T1::Output>, arg: Ptr<()>, value: T1::Output) {
		let this = arg.cast::<Self>().make_mut().as_mut();

		this.result_1 = Some(value);
		this.check_complete();
	}

	fn complete_2(_: RequestPtr<T2::Output>, arg: Ptr<()>, value: T2::Output) {
		let this = arg.cast::<Self>().make_mut().as_mut();

		this.result_2 = Some(value);
		this.check_complete();
	}

	fn new(task_1: T1, task_2: T2) -> Self {
		unsafe {
			/* request arg ptrs are assigned once pinned */
			Self {
				task_1: Some(task_1),
				req_1: Request::new(Ptr::null(), Self::complete_1),
				cancel_1: None,
				result_1: None,

				task_2: Some(task_2),
				req_2: Request::new(Ptr::null(), Self::complete_2),
				cancel_2: None,
				result_2: None,

				request: Ptr::null()
			}
		}
	}

	fn check_complete(&mut self) {
		if self.result_1.is_none() || self.result_2.is_none() {
			return;
		}

		/*
		 * Safety: cannot access `self` once a cancel or a complete is called,
		 * as it may be freed by the callee
		 */
		Request::complete(
			self.request,
			Join(self.result_1.take().unwrap(), self.result_2.take().unwrap())
		);
	}

	#[sync_task]
	fn join(&mut self) -> Join<T1::Output, T2::Output> {
		fn cancel(self: &mut Self) -> Result<()> {
			let (cancel_1, cancel_2) = unsafe {
				(
					self.cancel_1.take().unwrap().run(),
					self.cancel_2.take().unwrap().run()
				)
			};

			if cancel_1.is_err() {
				return cancel_1;
			}

			if cancel_2.is_err() {
				return cancel_2;
			}

			Ok(())
		}

		unsafe {
			match self.task_1.take().unwrap().run(Ptr::from(&self.req_1)) {
				Progress::Pending(cancel) => self.cancel_1 = Some(cancel),
				Progress::Done(value) => self.result_1 = Some(value)
			}

			match self.task_2.take().unwrap().run(Ptr::from(&self.req_2)) {
				Progress::Pending(cancel) => self.cancel_2 = Some(cancel),
				Progress::Done(value) => {
					if self.result_1.is_some() {
						return Progress::Done(Join(self.result_1.take().unwrap(), value));
					}

					self.result_2 = Some(value);
				}
			}

			self.request = request;

			return Progress::Pending(cancel(self, request));
		}
	}
}

impl<T1: SyncTask, T2: SyncTask> Global for JoinData<T1, T2> {
	unsafe fn pinned(&mut self) {
		let mut this = MutPtr::from(self);
		let arg = this.as_unit().into();

		this.req_1.set_arg(arg);
		this.req_2.set_arg(arg);
	}
}

#[async_fn]
pub async fn join_sync<T1: SyncTask, T2: SyncTask>(
	task_1: T1, task_2: T2
) -> Join<T1::Output, T2::Output> {
	let data = JoinData::new(task_1, task_2);

	pin_local_mut!(data);
	block_on(data.join()).await
}

/// Joins two tasks A and B and waits
/// for both of them to finish, returning
/// both of their results
#[async_fn]
pub async fn join<R: PerContextRuntime, T1: Task, T2: Task>(
	runtime: Handle<R>, task_1: T1, task_2: T2
) -> Join<T1::Output, T2::Output> {
	join_sync(
		spawn_sync_with_runtime(runtime, task_1),
		spawn_sync_with_runtime(runtime, task_2)
	)
	.await
}
