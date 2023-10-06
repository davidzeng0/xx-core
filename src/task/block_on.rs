use std::mem::MaybeUninit;

use super::{Cancel, Progress, Request, Task};

type ResumeArg<Resume, Output> = (Resume, Output);

fn block_resume<Resume: FnMut(), Output>(arg: *const (), value: Output) {
	let arg = unsafe { &mut *(arg as *mut ResumeArg<Resume, Output>) };

	arg.1 = value;
	(arg.0)();
}

#[inline(always)]
pub fn block_on<Block: FnMut(C), Resume: FnMut(), T: Task<Output, C>, C: Cancel, Output>(
	mut block: Block, resume: Resume, task: T
) -> Output {
	let mut arg: ResumeArg<Resume, Output> = (resume, unsafe {
		MaybeUninit::<Output>::uninit().assume_init()
	});

	unsafe {
		let request = Request::new(
			&mut arg as *mut _ as *const (),
			block_resume::<Resume, Output>
		);

		match task.run(&request) {
			Progress::Done(value) => return value,
			Progress::Pending(cancel) => block(cancel)
		};
	};

	arg.1
}
