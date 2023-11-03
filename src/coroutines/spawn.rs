use std::{marker::PhantomData, ptr::null};

use super::*;
use crate::{fiber::Start, pin_local_mut, task::Boxed, trace};

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
	fn worker_start(arg: *const ()) {
		let mut this: MutPtr<Self> = ConstPtr::from(arg).cast();

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

		trace!(target: &worker, "-- Exited");

		unsafe {
			worker.exit();
		}
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

		let start = Start::new(Self::worker_start, MutPtr::from(&mut data).as_raw_ptr());

		data.move_to_worker = Some((entry, task, executor.new_worker(start)));

		let worker = &mut data.move_to_worker.as_mut().unwrap().2;

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

/// Spawn a new fiber. The result of the fiber will be returned as a [`Task`]
///
/// [`Task`]: crate::task::Task
#[sync_task]
pub fn spawn_sync<R: PerContextRuntime, T: Task>(
	executor: Handle<Executor>, entry: impl FnOnce(Handle<Worker>) -> R, task: T
) -> T::Output {
	fn cancel(_context: Handle<Context>) -> Result<()> {
		/* use this fn to generate the cancel closure type */
		Ok(())
	}

	unsafe { SpawnWorker::spawn(executor, entry, task).run(request) }
}

/// Utility function that calls the above with the
/// executor and runtime passed to this function
#[sync_task]
pub fn spawn_sync_with_runtime<R: PerContextRuntime, T: Task>(
	mut runtime: Handle<R>, task: T
) -> T::Output {
	fn cancel(_context: Handle<Context>) -> Result<()> {
		Ok(())
	}

	let executor = runtime.executor();
	let new_runtime = |worker| runtime.new_from_worker(worker);

	unsafe { spawn_sync(executor, new_runtime, task).run(request) }
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
			let this = MutPtr::from(self);
			let this = unsafe { Boxed::from_raw(this.as_ptr_mut()) };

			drop(this);
		}
	}

	fn inc_ref(&mut self) {
		self.refs += 1;
	}

	fn spawn_complete(_: RequestPtr<Output>, arg: *const (), output: Output) {
		let mut this: MutPtr<Spawn<Output>> = ConstPtr::from(arg).cast();

		if this.waiter.is_null() {
			this.output = Some(output);
		} else {
			Request::complete(this.waiter, output);
		}

		this.dec_ref();
	}

	pub fn new() -> Self {
		unsafe {
			Self {
				request: Request::new(null(), Self::spawn_complete),
				cancel: None,
				waiter: RequestPtr::null(),
				output: None,
				refs: 0
			}
		}
	}

	#[async_fn]
	pub async fn run<R: PerContextRuntime, T: Task>(
		runtime: Handle<R>, task: T
	) -> JoinHandle<T::Output> {
		let mut this = Boxed::new(Spawn::new());

		unsafe {
			match spawn_sync_with_runtime(runtime, task).run(ConstPtr::from(&this.request)) {
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

		JoinHandle {
			task: MutPtr::from(unsafe { Boxed::into_raw(this) })
		}
	}

	pub fn cancel(&mut self) -> Result<()> {
		unsafe { self.cancel.take().unwrap().run() }
	}
}

impl<Output> Global for Spawn<Output> {
	unsafe fn pinned(&mut self) {
		let arg: MutPtr<Self> = self.into();

		self.request.set_arg(arg.as_raw_ptr());
	}
}

struct JoinTask<Output> {
	task: MutPtr<Spawn<Output>>
}

struct JoinCancel<Output> {
	task: MutPtr<Spawn<Output>>
}

pub struct JoinHandle<Output> {
	task: MutPtr<Spawn<Output>>
}

unsafe impl<Output> SyncTask for JoinTask<Output> {
	type Cancel = JoinCancel<Output>;
	type Output = Output;

	unsafe fn run(mut self, request: RequestPtr<Output>) -> Progress<Output, Self::Cancel> {
		self.task.waiter = request;

		/* we could make JoinTask return Progress::Done instead of checking below,
		 * but block_on is somewhat expensive, so only want to block on if
		 * it's necessary
		 */
		Progress::Pending(JoinCancel { task: self.task })
	}
}

unsafe impl<Output> Cancel for JoinCancel<Output> {
	unsafe fn run(mut self) -> Result<()> {
		self.task.cancel()
	}
}

impl<Output> Task for JoinHandle<Output> {
	type Output = Output;

	fn run(mut self, context: Handle<Context>) -> Output {
		if let Some(output) = self.task.output.take() {
			/* task finished inbetween spawn and await */
			return output;
		}

		block_on(JoinTask { task: self.task }).run(context)
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
#[async_fn]
pub async fn spawn<R: PerContextRuntime, T: Task + 'static>(
	runtime: Handle<R>, task: T
) -> JoinHandle<T::Output> {
	Spawn::<T::Output>::run(runtime, task).await
}
