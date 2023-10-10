use super::{Cancel, Progress, Request, RequestPtr, Task};
use crate::pointer::{ConstPtr, MutPtr};

type ResumeArg<Resume, Output> = (Resume, Option<Output>);

fn block_resume<Resume: FnMut(), Output>(_: RequestPtr<Output>, arg: *const (), value: Output) {
	let mut arg: MutPtr<ResumeArg<Resume, Output>> = ConstPtr::from(arg).cast();

	arg.1 = Some(value);
	(arg.0)();
}

#[inline(always)]
pub fn block_on<Block: FnMut(C), Resume: FnMut(), T: Task<Output, C>, C: Cancel, Output>(
	mut block: Block, resume: Resume, task: T
) -> Output {
	let mut arg: ResumeArg<Resume, Output> = (resume, None);

	unsafe {
		let request = Request::new(
			MutPtr::from(&mut arg).as_raw_ptr(),
			block_resume::<Resume, Output>
		);

		match task.run(ConstPtr::from(&request)) {
			Progress::Done(value) => return value,
			Progress::Pending(cancel) => block(cancel)
		};
	};

	arg.1
		.take()
		.expect("Task did not finish after blocking call ended")
}
