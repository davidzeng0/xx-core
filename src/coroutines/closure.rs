use super::{env::AsyncContext, task::AsyncTask};
use crate::{
	closure::{Closure, ClosureWrap},
	task::env::Handle
};

pub type AsyncClosureWrap<Inner, Context, Output> = ClosureWrap<Inner, Handle<Context>, Output>;

impl<Inner: FnOnce(Handle<Context>) -> Output, Context: AsyncContext, Output>
	AsyncTask<Context, Output> for AsyncClosureWrap<Inner, Context, Output>
{
	#[inline(always)]
	fn run(self, context: Handle<Context>) -> Output {
		self.call(context)
	}
}

pub type AsyncClosure<Context, Capture, Output> = Closure<Capture, Handle<Context>, Output>;

impl<Context: AsyncContext, Capture, Output> AsyncTask<Context, Output>
	for AsyncClosure<Context, Capture, Output>
{
	#[inline(always)]
	fn run(self, context: Handle<Context>) -> Output {
		self.call(context)
	}
}
