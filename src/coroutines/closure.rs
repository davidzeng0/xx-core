#![allow(clippy::module_name_repetitions)]

use super::*;
use crate::closure;

pub type OpaqueClosure<F, Output> = closure::OpaqueClosure<F, Ptr<Context>, Output>;

impl<F: FnOnce(Ptr<Context>) -> Output, Output> Task for OpaqueClosure<F, Output> {
	type Output = Output;

	#[asynchronous(task)]
	#[inline(always)]
	async fn run(self) -> Output {
		self.call(get_context().await)
	}
}

pub type Closure<Capture, Output> = closure::Closure<Capture, Ptr<Context>, Output>;

impl<Capture, Output> Task for Closure<Capture, Output> {
	type Output = Output;

	#[asynchronous(task)]
	#[inline(always)]
	async fn run(self) -> Output {
		self.call(get_context().await)
	}
}
