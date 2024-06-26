#![allow(clippy::module_name_repetitions)]

use super::*;
use crate::closure::*;

pub type FutureClosure<F, Output, Cancel> =
	OpaqueClosure<F, ReqPtr<Output>, Progress<Output, Cancel>>;

/* Safety: contract upheld by user of #[future] */
unsafe impl<F: FnOnce(ReqPtr<Output>) -> Progress<Output, C>, Output, C: Cancel> Future
	for FutureClosure<F, Output, C>
{
	type Cancel = C;
	type Output = Output;

	#[inline(always)]
	unsafe fn run(self, request: ReqPtr<Output>) -> Progress<Output, C> {
		self.call(request)
	}
}

pub type CancelClosure<Capture> = Closure<Capture, (), Result<()>>;

/* Safety: contract upheld by user of #[future] */
unsafe impl<Capture> Cancel for CancelClosure<Capture> {
	#[inline(always)]
	unsafe fn run(self) -> Result<()> {
		self.call(())
	}
}
