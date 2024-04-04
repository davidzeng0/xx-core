#![allow(clippy::module_name_repetitions)]

use super::*;
use crate::{closure, macros::macro_each};

pub type OpaqueClosure<F, Output, const INLINE: u32> =
	closure::OpaqueClosure<F, Ptr<Context>, Output, INLINE>;

macro_rules! impl_closure {
	($inline:ident) => {
		impl<F, Output> Task for OpaqueClosure<F, Output, { closure::$inline }>
		where
			F: FnOnce(Ptr<Context>) -> Output
		{
			type Output = Output;

			#[inline(always)]
			fn run(self, context: Ptr<Context>) -> Output {
				self.call(context)
			}
		}
	};
}

macro_each!(impl_closure, INLINE_NEVER, INLINE_DEFAULT, INLINE_ALWAYS);

pub type Closure<Capture, Output> = closure::Closure<Capture, Ptr<Context>, Output>;

impl<Capture, Output> Task for Closure<Capture, Output> {
	type Output = Output;

	#[inline(always)]
	fn run(self, context: Ptr<Context>) -> Output {
		self.call(context)
	}
}
