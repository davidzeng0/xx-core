pub mod closure;
pub mod context;
pub use context::*;
pub mod executor;
pub use executor::*;
pub mod worker;
pub use worker::*;
pub mod spawn;
pub use spawn::*;
pub mod select;
pub use select::*;
pub mod join;
pub use join::*;

pub use crate::macros::asynchronous;
use crate::{
	debug,
	error::*,
	fiber::*,
	future::{
		block_on::block_on as sync_block_on, closure::*, future, Cancel, Complete,
		Future as SyncTask, Progress, ReqPtr, Request
	},
	opt::hint::*,
	pointer::*
};

struct TaskHandle<T: SyncTask> {
	task: Option<T>,
	request: Request<T::Output>,
	cancel: Option<T::Cancel>,
	result: Option<T::Output>
}

impl<T: SyncTask> TaskHandle<T> {
	pub fn new(task: T, callback: Complete<T::Output>) -> Self {
		Self {
			task: Some(task),
			request: Request::new(Ptr::null(), callback),
			cancel: None,
			result: None
		}
	}

	pub unsafe fn run(&mut self) {
		match self.task.take().unwrap().run(Ptr::from(&self.request)) {
			Progress::Pending(cancel) => self.cancel = Some(cancel),
			Progress::Done(value) => self.complete(value)
		}
	}

	pub fn done(&self) -> bool {
		self.result.is_some()
	}

	pub fn complete(&mut self, result: T::Output) {
		self.cancel = None;
		self.result = Some(result);
	}

	pub fn take_result(&mut self) -> T::Output {
		self.result.take().unwrap()
	}

	pub unsafe fn try_cancel(&mut self) -> Option<Result<()>> {
		let result = self.cancel.take()?.run();

		if let Err(err) = &result {
			debug!("Cancel returned an error: {:?}", err);
		}

		Some(result)
	}

	pub unsafe fn cancel(&mut self) -> Result<()> {
		self.try_cancel().unwrap()
	}

	pub fn set_arg<A>(&mut self, arg: Ptr<A>) {
		self.request.set_arg(arg.as_unit())
	}
}

/// An async task
pub trait Task {
	type Output;

	fn run(self, context: Ptr<Context>) -> Self::Output;
}

#[asynchronous]
pub async fn get_context() -> Ptr<Context> {
	__xx_internal_async_context
}

pub unsafe fn with_context<T: Task>(context: Ptr<Context>, task: T) -> T::Output {
	context.as_ref().run(task)
}

#[asynchronous]
pub async fn block_on<T: SyncTask>(task: T) -> T::Output {
	unsafe { get_context().await.as_ref() }.block_on(task)
}

#[asynchronous]
pub async fn is_interrupted() -> bool {
	unsafe { get_context().await.as_ref() }.interrupted()
}

#[asynchronous]
pub async fn check_interrupt() -> Result<()> {
	if unlikely(is_interrupted().await) {
		Err(Core::Interrupted.new())
	} else {
		Ok(())
	}
}

#[asynchronous]
pub async fn clear_interrupt() {
	unsafe { get_context().await.as_ref() }.clear_interrupt()
}

#[asynchronous]
pub async fn take_interrupt() -> bool {
	let interrupted = is_interrupted().await;

	if unlikely(interrupted) {
		clear_interrupt().await;
	}

	interrupted
}

#[asynchronous]
pub async fn check_interrupt_take() -> Result<()> {
	if unlikely(take_interrupt().await) {
		Err(Core::Interrupted.new())
	} else {
		Ok(())
	}
}

/// Creates an interrupt guard
///
/// While this guard is held, any attempt to interrupt
/// the current context will be ignored
#[asynchronous]
pub async unsafe fn interrupt_guard() -> InterruptGuard {
	InterruptGuard::new(get_context().await)
}
