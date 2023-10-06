use super::{env::AsyncContext, task::AsyncTask};
use crate::{closure::Closure, task::env::Handle};

pub type AsyncClosure<Context, Capture, Output> = Closure<Capture, Handle<Context>, Output>;

impl<Context: AsyncContext, Capture: Sized, Output> AsyncTask<Context, Output>
	for AsyncClosure<Context, Capture, Output>
{
	#[inline(always)]
	fn run(self, context: Handle<Context>) -> Output {
		self.call(context)
	}
}
