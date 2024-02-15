use std::result;

use super::*;

#[derive(Debug)]
pub struct Join<O1, O2>(pub O1, pub O2);

impl<O1, O2, E> Join<result::Result<O1, E>, result::Result<O2, E>> {
	/// Flatten the `Join`, returning the first error it encounters
	pub fn flatten(self) -> result::Result<Join<O1, O2>, E> {
		Ok(Join(self.0?, self.1?))
	}
}

impl<O1, O2> Join<Option<O1>, Option<O2>> {
	/// Flatten the `Join`, returning none if there are any
	pub fn flatten(self) -> Option<Join<O1, O2>> {
		Some(Join(self.0?, self.1?))
	}
}

struct JoinData<T1: SyncTask, T2: SyncTask> {
	handle_1: TaskHandle<T1>,
	handle_2: TaskHandle<T2>,
	request: ReqPtr<Join<T1::Output, T2::Output>>
}

impl<T1: SyncTask, T2: SyncTask> JoinData<T1, T2> {
	unsafe fn complete_1(_: ReqPtr<T1::Output>, arg: Ptr<()>, value: T1::Output) {
		let this = arg.cast::<Self>().cast_mut().as_mut();

		this.handle_1.complete(value);
		this.check_complete();
	}

	unsafe fn complete_2(_: ReqPtr<T2::Output>, arg: Ptr<()>, value: T2::Output) {
		let this = arg.cast::<Self>().cast_mut().as_mut();

		this.handle_2.complete(value);
		this.check_complete();
	}

	fn new(task_1: T1, task_2: T2) -> Self {
		/* request arg ptrs are assigned once pinned */
		Self {
			handle_1: TaskHandle::new(task_1, Self::complete_1),
			handle_2: TaskHandle::new(task_2, Self::complete_2),
			request: Ptr::null()
		}
	}

	unsafe fn check_complete(&mut self) {
		if !self.handle_1.done() || !self.handle_2.done() {
			return;
		}

		/*
		 * Safety: cannot access `self` once a cancel or a complete is called,
		 * as it may be freed by the callee
		 */
		Request::complete(
			self.request,
			Join(self.handle_1.take_result(), self.handle_2.take_result())
		);
	}

	#[future]
	unsafe fn join(&mut self) -> Join<T1::Output, T2::Output> {
		#[cancel]
		fn cancel(self: &mut Self) -> Result<()> {
			let (cancel_1, cancel_2) =
				unsafe { (self.handle_1.try_cancel(), self.handle_2.try_cancel()) };

			if let Some(Err(result)) = cancel_1 {
				return Err(result);
			}

			if let Some(Err(result)) = cancel_2 {
				return Err(result);
			}

			Ok(())
		}

		unsafe {
			self.handle_1.run();
			self.handle_2.run();
		}

		if self.handle_1.done() && self.handle_2.done() {
			Progress::Done(Join(
				self.handle_1.take_result(),
				self.handle_2.take_result()
			))
		} else {
			self.request = request;

			Progress::Pending(cancel(self, request))
		}
	}
}

unsafe impl<T1: SyncTask, T2: SyncTask> Pin for JoinData<T1, T2> {
	unsafe fn pin(&mut self) {
		let arg = self.into();

		self.handle_1.set_arg(arg);
		self.handle_2.set_arg(arg);
	}
}

#[asynchronous]
pub async unsafe fn join_sync_task<T1: SyncTask, T2: SyncTask>(
	task_1: T1, task_2: T2
) -> Join<T1::Output, T2::Output> {
	let mut data = JoinData::new(task_1, task_2);

	block_on(data.pin_local().join()).await
}

/// Joins two tasks A and B and waits
/// for both of them to finish, returning
/// both of their results
#[asynchronous]
pub async unsafe fn join<R: Environment, T1: Task, T2: Task>(
	runtime: Ptr<R>, task_1: T1, task_2: T2
) -> Join<T1::Output, T2::Output> {
	join_sync_task(
		spawn_future_with_env(runtime, task_1),
		spawn_future_with_env(runtime, task_2)
	)
	.await
}
