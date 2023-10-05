use std::io::Result;

use super::task::AsyncTask;
use crate::task::{env::Global, Cancel, Task};

pub trait AsyncContext: Global + Sized {
	fn run<T: AsyncTask<Self, Output>, Output>(&mut self, task: T) -> Output;

	fn block_on<T: Task<Output, C>, C: Cancel, Output>(&mut self, task: T) -> Output;

	fn interrupt(&mut self) -> Result<()>;
}
