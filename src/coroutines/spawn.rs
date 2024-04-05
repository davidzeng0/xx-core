#![allow(clippy::multiple_unsafe_ops_per_block)]

use std::{cell::Cell, mem::replace, rc::Rc};

use super::*;
use crate::{runtime::PanickingResult, trace, warn};

#[allow(clippy::module_name_repetitions)]
pub type SpawnResult<T> = PanickingResult<T>;
enum SpawnData<E, T: Task> {
	Uninit,
	Start(E, T, Worker, ReqPtr<SpawnResult<T::Output>>),
	Pending(Ptr<Context>, Ptr<Cell<bool>>),
	Done(SpawnResult<T::Output>)
}

struct SpawnWorker<Entry, T: Task> {
	data: SpawnData<Entry, T>
}

#[future]
impl<Env: Environment, Entry: FnOnce(Ptr<Worker>) -> Env, T: Task> SpawnWorker<Entry, T> {
	/// # Safety
	/// `arg` must be dereferenceable as a &mut Self.
	/// Self::data must be a `SpawnData::Start`
	/// this function must be the entry point of a worker
	unsafe fn worker_start(arg: Ptr<()>) {
		let this = arg.cast::<Self>().cast_mut();

		let SpawnData::Start(entry, task, mut worker, request) =
			/* Safety: guaranteed by caller */
			replace(unsafe { &mut ptr!(this=>data) }, SpawnData::Uninit)
		else {
			/* Safety: guaranteed by caller */
			unsafe { unreachable_unchecked() };
		};

		trace!(target: &worker, "== Entered worker");

		{
			let worker = worker.pin_local();

			if let Some(sync_output) =
				Self::execute(this, ptr!(&*worker), entry, task, request).transpose()
			{
				/* Safety: `this` is still a valid pointer */
				unsafe { ptr!(this=>data = SpawnData::Done(sync_output)) };
			}
		}

		trace!(target: &worker, "== Completed");

		/* Safety: worker has completed */
		unsafe { worker.exit() };
	}

	#[inline(always)]
	fn execute(
		this: MutPtr<Self>, worker: Ptr<Worker>, entry: Entry, task: T,
		request: ReqPtr<SpawnResult<T::Output>>
	) -> SpawnResult<Option<T::Output>> {
		let environment = catch_unwind(AssertUnwindSafe(|| entry(worker))).map_err(|err| {
			warn!(
				/* Safety: logging */
				target: worker,
				"== Failed to start worker: panicked when trying to create a new context"
			);

			err
		})?;

		let context = environment.context();
		let is_async = Cell::new(false);

		/* pass back a pointer to is_async. the caller will tell us
		 * if we've suspended from this function
		 *
		 * cannot call Request::complete when synchronous,
		 * as that double resumes the calling worker, and violates the contract of
		 * Future
		 *
		 * Safety: we have mutable access here
		 */
		unsafe {
			ptr!(this=>data = SpawnData::Pending(ptr!(context), ptr!(&is_async)));
		}

		let result = catch_unwind(AssertUnwindSafe(|| context.run(task)));

		if is_async.get() {
			/* Safety: only called once
			 * Note: Request::complete does not panic
			 * Note: `self` is now dangling
			 */
			unsafe { Request::complete(request, result) };

			Ok(None)
		} else {
			result.map(Some)
		}
	}

	/// # Safety
	/// The executor, `task`, and the created runtime outlive the worker.
	#[future]
	unsafe fn spawn(executor: Ptr<Executor>, entry: Entry, task: T) -> SpawnResult<T::Output> {
		#[cancel]
		fn cancel(context: Ptr<Context>) -> Result<()> {
			/* Safety: guaranteed by Future's contract */
			unsafe { Context::interrupt(context) }
		}

		let mut spawn = Self { data: SpawnData::Uninit };

		/* Safety: worker_start never panics */
		let start = unsafe { Start::new(Self::worker_start, ptr!(&mut spawn).cast_const().cast()) };

		/* Safety: guaranteed by caller */
		let worker = unsafe { ptr!(executor=>new_worker(start)) };

		spawn.data = SpawnData::Start(entry, task, worker, request);

		let SpawnData::Start(_, _, worker, _) = &spawn.data else {
			/* Safety: we just assigned this */
			unsafe { unreachable_unchecked() };
		};

		/* Safety: the worker has not been started yet. the worker isn't exited until
		 * it completes */
		unsafe { ptr!(executor=>start(ptr!(worker))) };

		match replace(&mut spawn.data, SpawnData::Uninit) {
			SpawnData::Done(result) => Progress::Done(result),
			SpawnData::Pending(context, is_async) => {
				/* worker suspended without completing */

				/* Safety: pending task returns a valid pointer to a Cell */
				unsafe { ptr!(is_async=>set(true)) };

				Progress::Pending(cancel(context, request))
			}

			/* Safety: must be either pending or done */
			_ => unsafe { unreachable_unchecked() }
		}
	}
}

/// Spawn a new worker. The result of the worker will be returned as a
/// [`Future`]
///
/// # Safety
/// The executor, `task`, and the created runtime outlive the worker.
///
/// [`Future`]: crate::future::Future
#[future]
pub unsafe fn spawn_task<E, F, T>(
	executor: Ptr<Executor>, entry: F, task: T
) -> SpawnResult<T::Output>
where
	E: Environment,
	F: FnOnce(Ptr<Worker>) -> E,
	T: Task
{
	#[cancel]
	fn cancel(_context: Ptr<Context>) -> Result<()> {
		/* use this fn to generate the cancel closure type */
		Ok(())
	}

	/* Safety: guaranteed by caller */
	unsafe { SpawnWorker::spawn(executor, entry, task).run(request) }
}

/// Utility function that calls the above with the
/// executor and runtime passed to this function
///
/// # Safety
/// see above
/// `runtime` must be a valid pointer for the duration of the function. the
/// trait implementer must return a valid executor
#[future]
pub unsafe fn spawn_task_with_env<E, T>(runtime: Ptr<E>, task: T) -> SpawnResult<T::Output>
where
	E: Environment,
	T: Task
{
	#[cancel]
	fn cancel(_context: Ptr<Context>) -> Result<()> {
		Ok(())
	}

	/* Safety: runtime is valid and executor is valid */
	let executor = unsafe { ptr!(runtime=>executor()) };

	/* Safety: worker is exited when complete */
	let entry = |worker| unsafe { ptr!(runtime=>clone(worker)) };

	/* Safety: guaranteed by caller */
	unsafe { spawn_task(executor, entry, task).run(request) }
}

struct SpawnHandle<Output> {
	#[allow(clippy::type_complexity)]
	cancel: Option<CancelClosure<(Ptr<Context>, ReqPtr<SpawnResult<Output>>)>>,
	output: Option<SpawnResult<Output>>,
	waiter: ReqPtr<SpawnResult<Output>>
}

impl<Output> SpawnHandle<Output> {
	/// # Safety
	/// the future must be running
	unsafe fn try_cancel(&mut self) -> Option<Result<()>> {
		self.cancel.take().map(|cancel| {
			/* Safety: guaranteed by caller */
			unsafe { cancel.run() }
		})
	}
}

struct Spawn<Output> {
	request: Request<SpawnResult<Output>>,
	handle: UnsafeCell<SpawnHandle<Output>>
}

impl<Output> Spawn<Output> {
	unsafe fn spawn_complete(
		_: ReqPtr<SpawnResult<Output>>, arg: Ptr<()>, output: SpawnResult<Output>
	) {
		/* Safety: we called into_raw if this task was in progress */
		let this = unsafe { Rc::from_raw(arg.cast::<Self>().as_ptr()) };

		/* Safety: exclusive unsafe cell access */
		let handle = unsafe { this.handle.as_mut() };

		if handle.waiter.is_null() {
			handle.output = Some(output);
		} else {
			/* Safety: complete the future */
			unsafe { Request::complete(handle.waiter, output) };
		}
	}

	fn new() -> Self {
		/* Safety: spawn_complete does not panic */
		unsafe {
			/* request arg is assigned once pinned */
			Self {
				request: Request::new(Ptr::null(), Self::spawn_complete),
				handle: UnsafeCell::new(SpawnHandle {
					cancel: None,
					output: None,
					waiter: Ptr::null()
				})
			}
		}
	}

	/// # Safety
	/// `runtime` is a valid pointer to an env that returns a valid executor
	/// task must outlive its execution time
	/// the executor must outlive the task execution time
	unsafe fn run<E, T>(runtime: Ptr<E>, task: T) -> JoinHandle<Output>
	where
		E: Environment,
		T: Task<Output = Output>
	{
		/* Safety: we are never unpinned */
		let this = unsafe { Self::new().pin_rc().into_inner() };

		/* Safety: exclusive unsafe cell access */
		let handle = unsafe { this.handle.as_mut() };

		/* Safety: guaranteed by caller */
		match unsafe { spawn_task_with_env(runtime, task).run(ptr!(&this.request)) } {
			Progress::Done(result) => handle.output = Some(result),
			Progress::Pending(cancel) => {
				handle.cancel = Some(cancel);

				let _ = Rc::into_raw(this.clone());
			}
		}

		JoinHandle { task: this }
	}
}

impl<Output> Pin for Spawn<Output> {
	unsafe fn pin(&mut self) {
		let arg = ptr!(&*self).cast();

		self.request.set_arg(arg);
	}
}

pub struct JoinHandle<Output> {
	task: Rc<Spawn<Output>>
}

#[future]
#[asynchronous]
impl<Output> JoinHandle<Output> {
	/// # Safety
	/// caller must ensure an aliased &mut does not get created
	#[allow(clippy::mut_from_ref)]
	unsafe fn handle(&self) -> &mut SpawnHandle<Output> {
		/* Safety: guaranteed by caller */
		unsafe { self.task.handle.as_mut() }
	}

	#[future]
	fn join(self) -> SpawnResult<Output> {
		#[cancel]
		fn cancel(this: Self) -> Result<()> {
			/* Safety: exclusive unsafe cell access. we may already be cancelling */
			unsafe { this.handle().try_cancel() }.unwrap_or(Ok(()))
		}

		/* Safety: exclusive unsafe cell access */
		unsafe { self.handle().waiter = request };

		/* we could make JoinTask return Progress::Done instead of checking below,
		 * but we want to avoid calling future::block_on if possible
		 */
		Progress::Pending(cancel(self, request))
	}

	#[must_use]
	pub fn is_done(&self) -> bool {
		/* Safety: exclusive unsafe cell access */
		unsafe { self.handle().output.is_some() }
	}

	/// Signals the task to cancel, without waiting for the result
	pub fn request_cancel(&self) -> Result<()> {
		if self.is_done() {
			Ok(())
		} else {
			/* Safety: task is running */
			unsafe { self.handle().try_cancel() }.unwrap_or(Ok(()))
		}
	}

	pub async fn try_join(self) -> SpawnResult<Output> {
		/* Safety: exclusive unsafe cell access */
		if let Some(output) = unsafe { self.handle().output.take() } {
			/* task finished in-between spawn and await */
			output
		} else {
			block_on(self.join()).await
		}
	}

	/// Signals the task to cancel, waits for, and returns the result
	pub async fn cancel(self) -> Output {
		let result = self.request_cancel();

		if result.is_err() {
			warn!(target: &self, ">> Cancel returned an {:?}", result);
		}

		self.await
	}
}

#[asynchronous(task)]
impl<Output> Task for JoinHandle<Output> {
	type Output = Output;

	async fn run(self) -> Output {
		let result = self.try_join().await;

		runtime::join(result)
	}
}

/// Spawn a new async task
///
/// Returns a join handle which may be used to get the result from the task
///
/// # Safety
/// `runtime` is a valid pointer to an env that returns a valid executor
/// task must outlive its execution time
/// the executor must outlive the task execution time
pub unsafe fn spawn<E, T>(runtime: Ptr<E>, task: T) -> JoinHandle<T::Output>
where
	E: Environment,
	T: Task
{
	/* Safety: guaranteed by caller */
	unsafe { Spawn::<T::Output>::run(runtime, task) }
}
