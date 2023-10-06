use std::io::Result;

use super::{Cancel, Progress, Request, Task};
use crate::closure::Closure;

pub type TaskClosure<Capture, Output, Cancel> =
	Closure<Capture, *const Request<Output>, Progress<Output, Cancel>>;

unsafe impl<Capture: Sized, Output, C: Cancel> Task<Output, C> for TaskClosure<Capture, Output, C> {
	#[inline(always)]
	unsafe fn run(self, request: *const Request<Output>) -> Progress<Output, C> {
		self.call(request)
	}
}

pub type CancelClosure<Capture> = Closure<Capture, (), Result<()>>;

unsafe impl<Capture: Sized> Cancel for CancelClosure<Capture> {
	#[inline(always)]
	unsafe fn run(self) -> Result<()> {
		self.call(())
	}
}
