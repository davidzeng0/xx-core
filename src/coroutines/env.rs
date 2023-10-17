use super::task::AsyncTask;
use crate::{
	error::Result,
	task::{env::Global, Cancel, Task}
};

pub trait AsyncContext: Global + Sized {
	/// Runs async task `T`
	fn run<T: AsyncTask<Self, Output>, Output>(&mut self, task: T) -> Output;

	/// Runs and blocks on sync task `T`
	fn block_on<T: Task<Output, C>, C: Cancel, Output>(&mut self, task: T) -> Output;

	/// Interrupt the current running task
	fn interrupt(&mut self) -> Result<()>;

	/// Returns true if the worker has a pending interrupt
	fn interrupted(&self) -> bool;

	/// Clears any pending interrupts on the current worker
	fn clear_interrupt(&mut self);

	fn interrupt_guard(&mut self, count: i32);
}
