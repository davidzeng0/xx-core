use std::mem::{ManuallyDrop, MaybeUninit};

use super::*;

type ResumeArg<Resume, Output> = (ManuallyDrop<Resume>, MaybeUninit<Output>);

fn block_resume<Resume: FnOnce(), Output>(_: RequestPtr<Output>, arg: *const (), value: Output) {
	let mut arg: MutPtr<ResumeArg<Resume, Output>> = ConstPtr::from(arg).cast();
	let resume = unsafe { ManuallyDrop::take(&mut arg.0) };

	arg.1.write(value);

	resume();
}

/// Block on a sync task
///
/// `block` is a function that doesn't return until the task finishes,
/// and is called with the task's cancel handle
///
/// `resume` is a function that is called when the task finishes,
/// to signal to the `block`ing function that it should return
///
/// Safety: memory leak if `resume` is not called, aka the blocking function
/// exits and the task never finishes
#[inline(always)]
pub fn block_on<Block: FnOnce(T::Cancel), Resume: FnOnce(), T: Task>(
	block: Block, resume: Resume, task: T
) -> T::Output {
	let mut arg: ResumeArg<Resume, T::Output> = (ManuallyDrop::new(resume), MaybeUninit::uninit());

	unsafe {
		let request = Request::new(
			MutPtr::from(&mut arg).as_raw_ptr(),
			block_resume::<Resume, T::Output>
		);

		match task.run(ConstPtr::from(&request)) {
			Progress::Pending(cancel) => block(cancel),
			Progress::Done(value) => return value
		};
	};

	unsafe { arg.1.assume_init() }
}
