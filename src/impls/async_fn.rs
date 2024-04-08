#![allow(clippy::module_name_repetitions)]

use crate::{coroutines::*, pointer::*};

pub trait AsyncFn<Args> {
	type Output;

	fn call(&self, args: Args) -> impl Task<Output = Self::Output>;
}

pub trait AsyncFnMut<Args> {
	type Output;

	fn call_mut(&mut self, args: Args) -> impl Task<Output = Self::Output>;
}

pub trait AsyncFnOnce<Args> {
	type Output;

	fn call_once(self, args: Args) -> impl Task<Output = Self::Output>;
}

pub mod internal {
	use super::*;

	pub struct OpaqueAsyncFn<F, const T: usize>(pub F);

	impl<F: FnOnce(Args, Ptr<Context>) -> Output, Args, Output> AsyncFnOnce<Args>
		for OpaqueAsyncFn<F, 0>
	{
		type Output = Output;

		#[cfg(not(doc))]
		#[asynchronous(traitext)]
		#[inline(always)]
		async fn call_once(self, args: Args) -> Output {
			self.0(args, get_context().await)
		}

		#[cfg(doc)]
		fn call_once(self, args: Args) -> impl Task<Output = Self::Output> {}
	}

	impl<F: FnMut(Args, Ptr<Context>) -> Output, Args, Output> AsyncFnMut<Args>
		for OpaqueAsyncFn<F, 1>
	{
		type Output = Output;

		#[cfg(not(doc))]
		#[asynchronous(traitext)]
		#[inline(always)]
		async fn call_mut(&mut self, args: Args) -> Output {
			self.0(args, get_context().await)
		}

		#[cfg(doc)]
		fn call_mut(&mut self, args: Args) -> impl Task<Output = Self::Output> {}
	}

	impl<F: Fn(Args, Ptr<Context>) -> Output, Args, Output> AsyncFn<Args> for OpaqueAsyncFn<F, 2> {
		type Output = Output;

		#[cfg(not(doc))]
		#[asynchronous(traitext)]
		#[inline(always)]
		async fn call(&self, args: Args) -> Output {
			self.0(args, get_context().await)
		}

		#[cfg(doc)]
		fn call(&self, args: Args) -> impl Task<Output = Self::Output> {}
	}
}
