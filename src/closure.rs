#![allow(clippy::module_name_repetitions)]

use std::marker::PhantomData;

pub const INLINE_NEVER: u32 = 0;
pub const INLINE_DEFAULT: u32 = 1;
pub const INLINE_ALWAYS: u32 = 2;

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

pub struct OpaqueClosure<F, Args, Output, const INLINE: u32>(F, PhantomData<(Args, Output)>);

impl<F: FnOnce(Args) -> Output, Args, Output, const INLINE: u32>
	OpaqueClosure<F, Args, Output, INLINE>
{
	pub const fn new(func: F) -> Self {
		Self(func, PhantomData)
	}
}

impl<F: FnOnce(Args) -> Output, Args, Output> OpaqueClosure<F, Args, Output, INLINE_NEVER> {
	#[inline(never)]
	pub fn call(self, args: Args) -> Output {
		self.0(args)
	}
}

impl<F: FnOnce(Args) -> Output, Args, Output> OpaqueClosure<F, Args, Output, INLINE_DEFAULT> {
	pub fn call(self, args: Args) -> Output {
		self.0(args)
	}
}

impl<F: FnOnce(Args) -> Output, Args, Output> OpaqueClosure<F, Args, Output, INLINE_ALWAYS> {
	#[inline(always)]
	pub fn call(self, args: Args) -> Output {
		self.0(args)
	}
}
