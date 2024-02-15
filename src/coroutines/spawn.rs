use std::{cell::Cell, marker::PhantomData, rc::Rc};

use super::*;
use crate::{trace, warn};

struct SpawnWorker<R: Environment, Entry: FnOnce(Ptr<Worker>) -> R, T: Task> {
	move_to_worker: Option<(Entry, T, Worker)>,
	request: ReqPtr<T::Output>,

	/* pass back */
	result: Option<T::Output>,
	context: Ptr<Context>,
	is_async: Ptr<Cell<bool>>,

	phantom: PhantomData<R>
}

#[future]
impl<R: Environment, Entry: FnOnce(Ptr<Worker>) -> R, T: Task> SpawnWorker<R, Entry, T> {
	unsafe fn worker_start(arg: Ptr<()>) {
		let this = arg.cast::<Self>().cast_mut().as_mut();
		let (entry, task, mut worker) = this.move_to_worker.take().unwrap();
		let request = this.request;

		{
			let worker = worker.pin_local();

			let runtime = entry(Ptr::from(&*worker));
			let context = runtime.context();
			let is_async = Cell::new(false);

			/* pass back a pointer to is_async. only the caller will know
			 * if we've suspended from this function
			 *
			 * cannot call Request::complete when synchronous,
			 * as that double resumes the calling worker
			 */
			this.is_async = Ptr::from(&is_async);
			this.context = context.into();

			trace!(target: &worker, "++ Spawned");

			let result = context.run(task);

			if is_async.get() {
				unsafe { Request::complete(request, result) };
			} else {
				this.result = Some(result);
			}

			trace!(target: &worker, "-- Completed");
		}

		worker.exit();
	}

	#[future]
	unsafe fn spawn(executor: Ptr<Executor>, entry: Entry, task: T) -> T::Output {
		#[cancel]
		fn cancel(context: Ptr<Context>) -> Result<()> {
			context.as_ref().interrupt()
		}

		let mut data = Self {
			move_to_worker: None,
			request,
			context: Ptr::null(),
			result: None,
			is_async: Ptr::null(),
			phantom: PhantomData
		};

		let start = Start::new(Self::worker_start, MutPtr::from(&mut data).as_unit().into());
		let pass = (entry, task, executor.as_ref().new_worker(start));
		let worker = &mut data.move_to_worker.insert(pass).2;

		executor.as_ref().start(worker.into());

		if data.result.is_some() {
			Progress::Done(data.result.take().unwrap())
		} else {
			/* worker suspended without producing a result (aka completing) */
			data.is_async.as_ref().set(true);

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
#[future]
pub unsafe fn spawn_future<R: Environment, T: Task>(
	executor: Ptr<Executor>, entry: impl FnOnce(Ptr<Worker>) -> R, task: T
) -> T::Output {
	#[cancel]
	fn cancel(_context: Ptr<Context>) -> Result<()> {
		/* use this fn to generate the cancel closure type */
		Ok(())
	}

	SpawnWorker::spawn(executor, entry, task).run(request)
}

/// Utility function that calls the above with the
/// executor and runtime passed to this function
#[future]
pub unsafe fn spawn_future_with_env<R: Environment, T: Task>(
	runtime: Ptr<R>, task: T
) -> T::Output {
	#[cancel]
	fn cancel(_context: Ptr<Context>) -> Result<()> {
		Ok(())
	}

	let executor = runtime.as_ref().executor();
	let new_runtime = |worker| runtime.as_ref().clone(worker);

	spawn_future(executor, new_runtime, task).run(request)
}

struct Spawn<Output> {
	request: Request<Output>,
	output: Option<Output>,
	cancel: Option<CancelClosure<(Ptr<Context>, ReqPtr<Output>)>>,
	waiter: ReqPtr<Output>
}

type AsyncSpawn<Output> = Rc<UnsafeCell<Spawn<Output>>>;

impl<Output> Spawn<Output> {
	unsafe fn spawn_complete(_: ReqPtr<Output>, arg: Ptr<()>, output: Output) {
		let cell = arg.cast::<UnsafeCell<Spawn<Output>>>().cast_mut();
		let this = cell.as_ref().as_mut();

		if this.waiter.is_null() {
			this.output = Some(output);
		} else {
			Request::complete(this.waiter, output);
		}

		drop(unsafe { AsyncSpawn::<Output>::from_raw(cell.as_ptr()) });
	}

	fn new() -> Self {
		Self {
			request: Request::new(Ptr::null(), Self::spawn_complete),
			output: None,
			cancel: None,
			waiter: ReqPtr::null()
		}
	}

	unsafe fn run<R: Environment, T: Task>(runtime: Ptr<R>, task: T) -> JoinHandle<T::Output> {
		let cell = UnsafeCell::new(Spawn::new()).pin_rc().into_inner();
		let this = cell.as_mut();

		match spawn_future_with_env(runtime, task).run(Ptr::from(&this.request)) {
			Progress::Pending(cancel) => {
				this.cancel = Some(cancel);

				Rc::into_raw(cell.clone());
			}

			Progress::Done(result) => {
				this.output = Some(result);
			}
		}

		JoinHandle { task: cell }
	}

	unsafe fn cancel(&mut self) -> Result<()> {
		self.cancel.take().unwrap().run()
	}
}

unsafe impl<Output> Pin for Spawn<Output> {
	unsafe fn pin(&mut self) {
		let arg = Ptr::from(&*self).as_unit();

		self.request.set_arg(arg);
	}
}

pub struct JoinHandle<Output> {
	task: AsyncSpawn<Output>
}

#[future]
#[asynchronous]
impl<Output> JoinHandle<Output> {
	#[future]
	fn join(self) -> Output {
		#[cancel]
		fn cancel(task: AsyncSpawn<Output>) -> Result<()> {
			unsafe { task.as_mut().cancel() }
		}

		unsafe { self.task.as_mut().waiter = request };

		/* we could make JoinTask return Progress::Done instead of checking below,
		 * but we want to avoid calling task::block_on if possible
		 */
		Progress::Pending(cancel(self.task, request))
	}

	pub fn is_done(&self) -> bool {
		unsafe { self.task.get().as_ref().output.is_some() }
	}

	/// Signals the task to cancel, without waiting for the result
	///
	/// Safety: The async task referenced by this handle must not be currently
	/// running. Cannot call twice
	pub unsafe fn request_cancel(&mut self) -> Result<()> {
		if self.is_done() {
			Ok(())
		} else {
			self.task.as_mut().cancel()
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

	fn run(self, context: Ptr<Context>) -> Output {
		if let Some(output) = unsafe { self.task.as_mut().output.take() } {
			/* task finished inbetween spawn and await */
			return output;
		}

		block_on(self.join()).run(context)
	}
}

/// Spawn a new async task
///
/// Returns a join handle which may be used to get the result from the task
///
/// Safety: runtime and task outlive worker
pub unsafe fn spawn<R: Environment, T: Task>(runtime: Ptr<R>, task: T) -> JoinHandle<T::Output> {
	Spawn::<T::Output>::run(runtime, task)
}
