use super::*;

pub trait AsyncFnOnce<Args = ()> {
	type Output;

	fn call_once(self, args: Args) -> impl for<'ctx> Task<Output<'ctx> = Self::Output>;
}

pub trait AsyncFnMut<Args = ()> {
	type Output;

	fn call_mut(&mut self, args: Args) -> impl for<'ctx> Task<Output<'ctx> = Self::Output>;
}

pub trait AsyncFn<Args = ()> {
	type Output;

	fn call(&self, args: Args) -> impl for<'ctx> Task<Output<'ctx> = Self::Output>;
}
