use std::marker::PhantomData;

pub struct Closure<Capture, Args, Output> {
	capture: Capture,
	call: fn(Capture, Args) -> Output
}

impl<Capture, Args, Output> Closure<Capture, Args, Output> {
	pub const fn new(capture: Capture, call: fn(Capture, Args) -> Output) -> Self {
		Self { capture, call }
	}

	#[inline(always)]
	pub fn call(self, args: Args) -> Output {
		(self.call)(self.capture, args)
	}
}

pub struct OpaqueClosure<F, Args, Output>(F, PhantomData<(Args, Output)>);

impl<F: FnOnce(Args) -> Output, Args, Output> OpaqueClosure<F, Args, Output> {
	pub const fn new(func: F) -> Self {
		Self(func, PhantomData)
	}
}

impl<F: FnOnce(Args) -> Output, Args, Output> OpaqueClosure<F, Args, Output> {
	#[inline(always)]
	pub fn call(self, args: Args) -> Output {
		self.0(args)
	}
}
