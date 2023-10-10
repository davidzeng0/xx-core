use super::{env::AsyncContext, task::AsyncTask};
use crate::{closure::ClosureWrap, task::env::Handle};

pub type AsyncClosure<Inner, Context, Output> = ClosureWrap<Inner, Handle<Context>, Output>;

impl<Inner: FnOnce(Handle<Context>) -> Output, Context: AsyncContext, Output>
	AsyncTask<Context, Output> for AsyncClosure<Inner, Context, Output>
{
	#[inline(always)]
	fn run(self, context: Handle<Context>) -> Output {
		self.call(context)
	}
}
