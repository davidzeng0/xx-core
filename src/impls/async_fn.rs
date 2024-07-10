#![allow(clippy::module_name_repetitions)]

use crate::coroutines::*;

pub trait AsyncFnOnce<Args> {
	type Output;

	fn call_once(self, args: Args) -> impl for<'ctx> Task<Output<'ctx> = Self::Output>;
}

pub trait AsyncFnMut<Args> {
	type Output;

	fn call_mut(&mut self, args: Args) -> impl for<'ctx> Task<Output<'ctx> = Self::Output>;
}

pub trait AsyncFn<Args> {
	type Output;

	fn call(&self, args: Args) -> impl for<'ctx> Task<Output<'ctx> = Self::Output>;
}

impl<F, T, Args, Output> AsyncFnOnce<Args> for F
where
	F: FnOnce(Args) -> T,
	T: for<'a> Task<Output<'a> = Output>
{
	type Output = Output;

	fn call_once(self, args: Args) -> impl for<'ctx> Task<Output<'ctx> = Self::Output> {
		self(args)
	}
}

impl<F, T, Args, Output> AsyncFnMut<Args> for F
where
	F: FnMut(Args) -> T,
	T: for<'a> Task<Output<'a> = Output>
{
	type Output = Output;

	fn call_mut(&mut self, args: Args) -> impl for<'ctx> Task<Output<'ctx> = Self::Output> {
		self(args)
	}
}

impl<F, T, Args, Output> AsyncFn<Args> for F
where
	F: Fn(Args) -> T,
	T: for<'a> Task<Output<'a> = Output>
{
	type Output = Output;

	fn call(&self, args: Args) -> impl for<'ctx> Task<Output<'ctx> = Self::Output> {
		self(args)
	}
}
