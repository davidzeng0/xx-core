use xx_core_macros::sync_task;

use super::{env::AsyncContext, executor::Executor, task::AsyncTask, worker::Worker};
use crate::{
	error::Result,
	fiber::Start,
	pin_local_mut,
	pointer::{ConstPtr, MutPtr},
	task::{env::Handle, Progress, Request, RequestPtr, Task},
	trace, xx_core
};

struct SpawnWorker<
	Context: AsyncContext,
	Entry: FnOnce(Handle<Worker>) -> (Context, Task),
	Task: AsyncTask<Context, Output>,
	Output
> {
	request: RequestPtr<Output>,
	worker: Option<Worker>,
	entry: Option<Entry>,
	result: Option<Output>,
	context: Handle<Context>,
	is_async: MutPtr<bool>
}

impl<
		Context: AsyncContext,
		Entry: FnOnce(Handle<Worker>) -> (Context, Task),
		Task: AsyncTask<Context, Output>,
		Output
	> SpawnWorker<Context, Entry, Task, Output>
{
	extern "C" fn worker_start(arg: *const ()) {
		let mut data: MutPtr<Self> = ConstPtr::from(arg).cast();
		let worker = data.worker.take().unwrap();

		pin_local_mut!(worker);

		let request = data.request;
		let mut is_async = false;

		let (mut context, task) = (data.entry.take().unwrap())((&mut worker).into());

		data.is_async = MutPtr::from(&mut is_async);
		data.context = (&mut context).into();

		trace!(target: &worker, "++ Spawned");

		let result = context.run(task);

		if is_async {
			Request::complete(request, result);
		} else {
			data.result = Some(result);
		}

		trace!(target: &worker, "-- Exited");

		unsafe {
			worker.exit();
		}
	}

	#[sync_task]
	fn spawn(mut executor: Handle<Executor>, entry: Entry) -> Output {
		fn cancel(mut context: Handle<Context>) -> Result<()> {
			context.interrupt()
		}

		let mut data = Self {
			request,
			worker: None,
			entry: Some(entry),
			context: unsafe { Handle::new_null() },
			result: None,
			is_async: MutPtr::null()
		};

		let start = Start::new(Self::worker_start, MutPtr::from(&mut data).as_raw_ptr());

		data.worker = Some(executor.new_worker(start));

		unsafe {
			executor.start(data.worker.as_mut().unwrap().into());
		}

		if data.result.is_some() {
			Progress::Done(data.result.take().unwrap())
		} else {
			*data.is_async.as_mut() = true;

			Progress::Pending(cancel(data.context, request))
		}
	}
}

/// Spawn a new fiber. The result of the fiber will be returned as a [`Task`]
///
/// [`Task`]: crate::task::Task
#[sync_task]
pub fn spawn<
	Context: AsyncContext,
	Entry: FnOnce(Handle<Worker>) -> (Context, Task),
	Task: AsyncTask<Context, Output>,
	Output
>(
	executor: Handle<Executor>, entry: Entry
) -> Output {
	fn cancel(_context: Handle<Context>) -> Result<()> {
		/* use this fn to generate the cancel closure type */
		Ok(())
	}

	let task = SpawnWorker::spawn(executor, entry);

	unsafe { task.run(request) }
}
