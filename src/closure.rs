pub struct Closure<Capture: Sized, Args: Sized, Output> {
	capture: Capture,
	call: fn(Capture, Args) -> Output
}

impl<Capture: Sized, Args: Sized, Output> Closure<Capture, Args, Output> {
	pub fn new(capture: Capture, call: fn(Capture, Args) -> Output) -> Self {
		Self { capture, call }
	}

	#[inline(always)]
	pub fn call(self, args: Args) -> Output {
		(self.call)(self.capture, args)
	}
}
