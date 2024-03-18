#![allow(clippy::multiple_unsafe_ops_per_block)]

use std::{cell::Cell, marker::PhantomData, mem::replace, rc::Rc};

use super::*;
use crate::{macros::unwrap_panic, trace, warn};

#[allow(clippy::module_name_repetitions)]
pub type SpawnResult<T> = PanickingResult<T>;

enum SpawnData<E, T: Task> {
	Uninit,
	Start(E, T, Worker, ReqPtr<SpawnResult<T::Output>>),
	Pending(Ptr<Context>, Ptr<Cell<bool>>),
	Done(SpawnResult<T::Output>)
}

struct SpawnWorker<E: Environment, Entry: FnOnce(Ptr<Worker>) -> E, T: Task> {
	data: SpawnData<Entry, T>,
	phantom: PhantomData<E>
}

#[future]
impl<E: Environment, Entry: FnOnce(Ptr<Worker>) -> E, T: Task> SpawnWorker<E, Entry, T> {
	/// # Safety
	/// `arg` must be dereferenceable as a &mut Self.
	/// Self::data must be a `SpawnData::Start`
	/// this function must be the entry point of a worker
	unsafe fn worker_start(arg: Ptr<()>) {
		/* Safety: guaranteed by caller */
		let this = unsafe { arg.cast::<Self>().cast_mut().as_mut() };

		let SpawnData::Start(entry, task, mut worker, request) =
			replace(&mut this.data, SpawnData::Uninit)
		else {
			/* Safety: guaranteed by caller */
			unsafe { unreachable_unchecked() };
		};

		trace!(target: &worker, "== Entered worker");

		{
			let worker = worker.pin_local();

			if let Some(sync_output) = this
				.execute(Ptr::from(&*worker), entry, task, request)
				.transpose()
			{
				this.data = SpawnData::Done(sync_output);
			}
		}

		trace!(target: &worker, "== Completed");

		/* Safety: worker has completed */
		unsafe { worker.exit() };
	}

	#[inline(always)]
	fn execute(
		&mut self, worker: Ptr<Worker>, entry: Entry, task: T,
		request: ReqPtr<SpawnResult<T::Output>>
	) -> SpawnResult<Option<T::Output>> {
		let environment = match catch_unwind(AssertUnwindSafe(|| entry(worker))) {
			Ok(ok) => ok,
			Err(err) => {
				warn!(
					/* Safety: logging */
					target: unsafe { worker.as_ref() },
					"== Failed to start worker: panicked when trying to create a new context"
				);

				return Err(err);
			}
		};

		let context = environment.context();
		let is_async = Cell::new(false);

		/* pass back a pointer to is_async. the caller will tell us
		 * if we've suspended from this function
		 *
		 * cannot call Request::complete when synchronous,
		 * as that double resumes the calling worker, and violates the contract of
		 * Future
		 */
		self.data = SpawnData::Pending(Ptr::from(context), Ptr::from(&is_async));

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
			unsafe { context.as_ref().interrupt() }
		}

		let mut spawn = Self { data: SpawnData::Uninit, phantom: PhantomData };

		/* Safety: worker_start never panics */
		let start = unsafe {
			Start::new(
				Self::worker_start,
				MutPtr::from(&mut spawn).as_unit().into()
			)
		};

		/* Safety: guaranteed by caller */
		let worker = unsafe { executor.as_ref().new_worker(start) };

		spawn.data = SpawnData::Start(entry, task, worker, request);

		let SpawnData::Start(_, _, worker, _) = &spawn.data else {
			/* Safety: we just assigned this */
			unsafe { unreachable_unchecked() };
		};

		/* Safety: the worker has not been started yet. the worker isn't exited until
		 * it completes */
		unsafe { executor.as_ref().start(worker.into()) };

		match replace(&mut spawn.data, SpawnData::Uninit) {
			SpawnData::Done(result) => Progress::Done(result),
			SpawnData::Pending(context, is_async) => {
				/* worker suspended without completing */

				/* Safety: pending task returns a valid pointer to a Cell */
				unsafe { is_async.as_ref() }.set(true);

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
	let executor = unsafe { runtime.as_ref().executor() };

	/* Safety: worker is exited when complete */
	let entry = |worker| unsafe { runtime.as_ref().clone(worker) };

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
		match unsafe { spawn_task_with_env(runtime, task).run(Ptr::from(&this.request)) } {
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
		let arg = Ptr::from(&*self).as_unit();

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

	/// Signals the task to cancel, waits for, and returns the result
	pub async fn cancel(self) -> Output {
		let result = self.request_cancel();

		if result.is_err() {
			warn!(target: &self, ">> Cancel returned an {:?}", result);
		}

		self.await
	}
}

impl<Output> Task for JoinHandle<Output> {
	type Output = Output;

	fn run(self, context: Ptr<Context>) -> Output {
		/* Safety: exclusive unsafe cell access */
		let result = if let Some(output) = unsafe { self.handle().output.take() } {
			/* task finished in-between spawn and await */
			output
		} else {
			/* Safety: we are in an async function */
			unsafe { with_context(context, block_on(self.join())) }
		};

		unwrap_panic!(result)
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
