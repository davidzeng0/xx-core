use std::{io::Result, mem::ManuallyDrop, ops::DerefMut};

use xx_core_macros::sync_task;

use super::{env::AsyncContext, executor::Executor, task::AsyncTask, worker::Worker};
use crate::task::{env::Handle, Progress, Request};

mod xx_core {
	pub use crate::*;
}

struct SpawnData<F, Output, Context: AsyncContext> {
	request: *const Request<Output>,
	worker: ManuallyDrop<Worker>,
	entry: F,
	result: Option<Output>,
	context: Handle<Context>,
	is_async: *mut bool
}

extern "C" fn worker_start<
	Context: AsyncContext,
	F: Fn(Handle<Worker>) -> (Context, Task),
	Task: AsyncTask<Context, Output>,
	Output
>(
	arg: *const ()
) {
	let data = unsafe { &mut *(arg as *mut SpawnData<F, Output, Context>) };
	let mut worker = unsafe { ManuallyDrop::take(&mut data.worker) };
	let request = data.request;

	let (mut context, task) = (data.entry)((&mut worker).into());
	let mut is_async = false;

	data.is_async = &mut is_async;
	data.context = (&mut context).into();

	let result = context.run(task);

	if is_async {
		Request::complete(request, result);
	} else {
		data.result = Some(result);
	}

	unsafe {
		worker.suspend();
	}

	panic!("Fiber resumed after completing")
}

/// Spawn a new fiber. The result of the fiber will be returned as a [`Task`]
///
/// [`Task`]: crate::task::Task
#[sync_task]
pub fn spawn<
	Context: AsyncContext,
	F: Fn(Handle<Worker>) -> (Context, Task),
	Task: AsyncTask<Context, Output>,
	Output
>(
	mut executor: Handle<Executor>, entry: F
) -> Output {
	fn cancel(mut context: Handle<Context>) -> Result<()> {
		context.interrupt()
	}

	let worker = ManuallyDrop::new(Worker::new(executor));
	let mut data = SpawnData {
		request,
		worker,
		entry,
		context: unsafe { Handle::new_empty() },
		result: None,
		is_async: 0 as *mut _
	};

	unsafe {
		executor.start(
			data.worker.deref_mut().into(),
			worker_start::<Context, F, Task, Output>,
			&mut data as *mut _ as *const ()
		);
	}

	if data.result.is_some() {
		Progress::Done(data.result.take().unwrap())
	} else {
		unsafe {
			*data.is_async = true;
		}

		Progress::Pending(cancel(data.context, request))
	}
}
