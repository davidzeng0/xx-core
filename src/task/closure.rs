use super::{Cancel, Progress, RequestPtr, Task};
use crate::{
	closure::{Closure, ClosureWrap},
	error::Result
};

pub type TaskClosureWrap<Inner, Output, Cancel> =
	ClosureWrap<Inner, RequestPtr<Output>, Progress<Output, Cancel>>;

unsafe impl<Inner: FnOnce(RequestPtr<Output>) -> Progress<Output, C>, Output, C: Cancel>
	Task<Output, C> for TaskClosureWrap<Inner, Output, C>
{
	#[inline(always)]
	unsafe fn run(self, request: RequestPtr<Output>) -> Progress<Output, C> {
		self.call(request)
	}
}

pub type CancelClosure<Capture> = Closure<Capture, (), Result<()>>;

unsafe impl<Capture> Cancel for CancelClosure<Capture> {
	#[inline(always)]
	unsafe fn run(self) -> Result<()> {
		self.call(())
	}
}
