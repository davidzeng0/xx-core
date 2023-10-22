use std::marker::PhantomData;

pub struct Closure<Capture, Args, Output> {
	capture: Capture,
	call: fn(Capture, Args) -> Output
}

impl<Capture, Args, Output> Closure<Capture, Args, Output> {
	#[inline(always)]
	pub const fn new(capture: Capture, call: fn(Capture, Args) -> Output) -> Self {
		Self { capture, call }
	}

	#[inline(always)]
	pub fn call(self, args: Args) -> Output {
		(self.call)(self.capture, args)
	}
}

pub struct ClosureWrap<Inner: FnOnce(Args) -> Output, Args, Output> {
	inner: Inner,
	phantom: PhantomData<(Args, Output)>
}

impl<Inner: FnOnce(Args) -> Output, Args, Output> ClosureWrap<Inner, Args, Output> {
	#[inline(always)]
	pub const fn new(inner: Inner) -> Self {
		Self { inner, phantom: PhantomData }
	}

	#[inline(always)]
	pub fn call(self, args: Args) -> Output {
		(self.inner)(args)
	}
}

pub mod lifetime {
	pub trait Captures<'__> {}

	impl<T: ?Sized> Captures<'_> for T {}
}
