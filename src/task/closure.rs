use super::*;
use crate::closure::*;

pub type TaskClosureWrap<Inner, Output, Cancel> =
	OpaqueClosure<Inner, ReqPtr<Output>, Progress<Output, Cancel>>;

unsafe impl<Inner: FnOnce(ReqPtr<Output>) -> Progress<Output, C>, Output, C: Cancel> Task
	for TaskClosureWrap<Inner, Output, C>
{
	type Cancel = C;
	type Output = Output;

	unsafe fn run(self, request: ReqPtr<Output>) -> Progress<Output, C> {
		self.call(request)
	}
}

pub type CancelClosure<Capture> = Closure<Capture, (), Result<()>>;

unsafe impl<Capture> Cancel for CancelClosure<Capture> {
	unsafe fn run(self) -> Result<()> {
		self.call(())
	}
}
