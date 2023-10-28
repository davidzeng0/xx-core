use super::*;
use crate::closure;

pub type ClosureWrap<Inner, Output> = closure::ClosureWrap<Inner, Handle<Context>, Output>;

impl<Inner: FnOnce(Handle<Context>) -> Output, Output> Task for ClosureWrap<Inner, Output> {
	type Output = Output;

	#[inline(always)]
	fn run(self, context: Handle<Context>) -> Output {
		self.call(context)
	}
}

pub type Closure<Capture, Output> = closure::Closure<Capture, Handle<Context>, Output>;

impl<Capture, Output> Task for Closure<Capture, Output> {
	type Output = Output;

	#[inline(always)]
	fn run(self, context: Handle<Context>) -> Output {
		self.call(context)
	}
}
