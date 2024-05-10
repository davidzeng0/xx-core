#![allow(clippy::module_name_repetitions)]

use crate::coroutines::*;

pub trait AsyncFnOnce<Args> {
	type Output;

	fn call_once(self, args: Args) -> impl for<'a> Task<Output<'a> = Self::Output>;
}

pub trait AsyncFnMut<Args> {
	type Output;

	fn call_mut(&mut self, args: Args) -> impl for<'a> Task<Output<'a> = Self::Output>;
}

pub trait AsyncFn<Args> {
	type Output;

	fn call(&self, args: Args) -> impl for<'a> Task<Output<'a> = Self::Output>;
}
