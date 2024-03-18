#![allow(clippy::module_name_repetitions)]

use std::marker::PhantomData;

pub struct Closure<Capture, Args, Output> {
	capture: Capture,
	call: fn(Capture, Args) -> Output
}

impl<Capture, Args, Output> Closure<Capture, Args, Output> {
	pub const fn new(capture: Capture, call: fn(Capture, Args) -> Output) -> Self {
		Self { capture, call }
	}

	pub fn call(self, args: Args) -> Output {
		(self.call)(self.capture, args)
	}
}

pub struct OpaqueClosure<Inner: FnOnce(Args) -> Output, Args, Output> {
	inner: Inner,
	phantom: PhantomData<(Args, Output)>
}

impl<Inner: FnOnce(Args) -> Output, Args, Output> OpaqueClosure<Inner, Args, Output> {
	pub const fn new(inner: Inner) -> Self {
		Self { inner, phantom: PhantomData }
	}

	pub fn call(self, args: Args) -> Output {
		(self.inner)(args)
	}
}
