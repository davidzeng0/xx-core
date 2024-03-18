#![allow(clippy::module_name_repetitions)]

use super::*;
use crate::closure;

pub type OpaqueClosure<Inner, Output> = closure::OpaqueClosure<Inner, Ptr<Context>, Output>;

impl<Inner, Output> Task for OpaqueClosure<Inner, Output>
where
	Inner: FnOnce(Ptr<Context>) -> Output
{
	type Output = Output;

	fn run(self, context: Ptr<Context>) -> Output {
		self.call(context)
	}
}

pub type Closure<Capture, Output> = closure::Closure<Capture, Ptr<Context>, Output>;

impl<Capture, Output> Task for Closure<Capture, Output> {
	type Output = Output;

	fn run(self, context: Ptr<Context>) -> Output {
		self.call(context)
	}
}
