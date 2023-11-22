use std::marker::PhantomData;

use super::*;
use crate::{trace, warn};

struct SpawnWorker<R: PerContextRuntime, Entry: FnOnce(Handle<Worker>) -> R, T: Task> {
	move_to_worker: Option<(Entry, T, Worker)>,
	request: RequestPtr<T::Output>,

	/* pass back */
	result: Option<T::Output>,
	context: Handle<Context>,
	is_async: MutPtr<bool>,

	phantom: PhantomData<R>
}

impl<R: PerContextRuntime, Entry: FnOnce(Handle<Worker>) -> R, T: Task> SpawnWorker<R, Entry, T> {
	fn worker_start(arg: Ptr<()>) {
		let this = arg.cast::<Self>().make_mut().as_mut();

		let (entry, task, worker) = this.move_to_worker.take().unwrap();
		let request = this.request;

		pin_local_mut!(worker);

		let mut runtime = entry((&mut worker).into());

		let mut is_async = false;
		let context = runtime.context();

		/* pass back a pointer to is_async. only the caller will know
		 * if we've suspended from this function
		 *
		 * cannot call Request::complete when synchronous,
		 * as that double resumes the calling worker
		 */
		this.is_async = MutPtr::from(&mut is_async);
		this.context = context.into();

		trace!(target: &worker, "++ Spawned");

		let result = context.run(task);

		if is_async {
			Request::complete(request, result);
		} else {
			this.result = Some(result);
		}

		trace!(target: &worker, "-- Completed");

		unsafe { worker.exit() };
	}

	#[sync_task]
	fn spawn(mut executor: Handle<Executor>, entry: Entry, task: T) -> T::Output {
		fn cancel(mut context: Handle<Context>) -> Result<()> {
			context.interrupt()
		}

		let mut data = Self {
			move_to_worker: None,
			request,
			context: unsafe { Handle::null() },
			result: None,
			is_async: MutPtr::null(),
			phantom: PhantomData
		};

		let start = Start::new(Self::worker_start, MutPtr::from(&mut data).as_unit().into());
		let pass = (entry, task, executor.new_worker(start));
		let worker = &mut data.move_to_worker.insert(pass).2;

		unsafe {
			executor.start(worker.into());
		}

		if data.result.is_some() {
			Progress::Done(data.result.take().unwrap())
		} else {
			/* worker suspended without producing a result (aka completing) */
			*data.is_async.as_mut() = true;

			Progress::Pending(cancel(data.context, request))
		}
	}
}

/// Spawn a new worker. The result of the worker will be returned as a [`Task`]
///
/// Safety: executor, task, and the created runtime outlive the worker. Sync
/// task safety requirements must also be met
///
/// [`Task`]: crate::task::Task
#[sync_task]
pub unsafe fn spawn_sync<R: PerContextRuntime, T: Task>(
	executor: Handle<Executor>, entry: impl FnOnce(Handle<Worker>) -> R, task: T
) -> T::Output {
	fn cancel(_context: Handle<Context>) -> Result<()> {
		/* use this fn to generate the cancel closure type */
		Ok(())
	}

	SpawnWorker::spawn(executor, entry, task).run(request)
}

/// Utility function that calls the above with the
/// executor and runtime passed to this function
#[sync_task]
pub unsafe fn spawn_sync_with_runtime<R: PerContextRuntime, T: Task>(
	mut runtime: Handle<R>, task: T
) -> T::Output {
	fn cancel(_context: Handle<Context>) -> Result<()> {
		Ok(())
	}

	let executor = runtime.executor();
	let new_runtime = |worker| runtime.new_from_worker(worker);

	spawn_sync(executor, new_runtime, task).run(request)
}

struct Spawn<Output> {
	request: Request<Output>,
	cancel: Option<CancelClosure<(Handle<Context>, RequestPtr<Output>)>>,
	waiter: RequestPtr<Output>,
	output: Option<Output>,
	refs: u32
}

impl<Output> Spawn<Output> {
	fn dec_ref(&mut self) {
		self.refs -= 1;

		if self.refs == 0 {
			drop(unsafe { Boxed::from_raw(self.into()) });
		}
	}

	fn inc_ref(&mut self) {
		self.refs += 1;
	}

	fn spawn_complete(_: RequestPtr<Output>, arg: Ptr<()>, output: Output) {
		let this = arg.cast::<Self>().make_mut().as_mut();

		if this.waiter.is_null() {
			this.output = Some(output);
		} else {
			Request::complete(this.waiter, output);
		}

		this.dec_ref();
	}

	fn new() -> Self {
		unsafe {
			Self {
				request: Request::new(Ptr::null(), Self::spawn_complete),
				cancel: None,
				waiter: RequestPtr::null(),
				output: None,
				refs: 0
			}
		}
	}

	fn run<R: PerContextRuntime, T: Task>(runtime: Handle<R>, task: T) -> JoinHandle<T::Output> {
		let mut this = Boxed::new(Spawn::new());

		unsafe {
			match spawn_sync_with_runtime(runtime, task).run(Ptr::from(&this.request)) {
				Progress::Pending(cancel) => {
					this.cancel = Some(cancel);
					this.inc_ref();
				}

				Progress::Done(result) => {
					this.output = Some(result);
				}
			}
		}

		this.inc_ref();

		JoinHandle { task: unsafe { Boxed::into_raw(this) }.into() }
	}

	unsafe fn cancel(&mut self) -> Result<()> {
		self.cancel.take().unwrap().run()
	}
}

impl<Output> Global for Spawn<Output> {
	unsafe fn pinned(&mut self) {
		let mut this = MutPtr::from(self);
		let arg = this.as_unit().into();

		this.request.set_arg(arg);
	}
}

pub struct JoinHandle<Output> {
	task: Handle<Spawn<Output>>
}

#[async_fn]
impl<Output> JoinHandle<Output> {
	#[sync_task]
	fn join(mut self) -> Output {
		fn cancel(mut task: Handle<Spawn<Output>>) -> Result<()> {
			unsafe { task.cancel() }
		}

		self.task.waiter = request;

		/* we could make JoinTask return Progress::Done instead of checking below,
		 * but we want to avoid calling task::block_on if possible
		 */
		Progress::Pending(cancel(self.task, request))
	}

	pub fn is_done(&self) -> bool {
		self.task.clone().output.is_some()
	}

	/// Signals the task to cancel, without waiting for the result
	///
	/// Safety: The async task referenced by this handle must not be currently
	/// running. Cannot call twice
	pub unsafe fn request_cancel(&mut self) -> Result<()> {
		if self.is_done() {
			Ok(())
		} else {
			self.task.cancel()
		}
	}

	/// Signals the task to cancel, without waiting for the result
	///
	/// Safety: see above function
	pub unsafe fn async_cancel(mut self) -> Result<()> {
		unsafe { self.request_cancel() }
	}

	/// Signals the task to cancel, waits for, and returns the result
	///
	/// Safety: see above function
	pub async unsafe fn cancel(mut self) -> Output {
		let result = unsafe { self.request_cancel() };

		if result.is_err() {
			warn!("Cancel returned an {:?}", result);
		}

		self.await
	}
}

impl<Output> Task for JoinHandle<Output> {
	type Output = Output;

	fn run(mut self, context: Handle<Context>) -> Output {
		if let Some(output) = self.task.output.take() {
			/* task finished inbetween spawn and await */
			return output;
		}

		block_on(self.join()).run(context)
	}
}

impl<Output> Drop for JoinHandle<Output> {
	fn drop(&mut self) {
		self.task.dec_ref();
	}
}

/// Spawn a new async task
///
/// Returns a join handle which may be used to get the result from the task
///
/// Safety: runtime and task outlive worker
pub unsafe fn spawn<R: PerContextRuntime, T: Task>(
	runtime: Handle<R>, task: T
) -> JoinHandle<T::Output> {
	Spawn::<T::Output>::run(runtime, task)
}
