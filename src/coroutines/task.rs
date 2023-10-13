use super::env::AsyncContext;
use crate::task::env::Handle;

pub trait AsyncTask<Context: AsyncContext, Output> {
	fn run(self, context: Handle<Context>) -> Output;
}
