use std::mem::replace;

use super::*;

enum BlockState<Resume, Output> {
	Pending(Resume),
	Done(Output)
}

unsafe fn block_resume<Resume: FnOnce(), Output>(_: ReqPtr<Output>, arg: Ptr<()>, value: Output) {
	let arg = arg.cast::<BlockState<Resume, Output>>().cast_mut().as_mut();
	let resume = replace(arg, BlockState::Done(value));

	let BlockState::Pending(resume) = resume else {
		panic!("Double resume detected");
	};

	resume();
}

/// Block on a sync task
///
/// `block` is a function that doesn't return until the task finishes,
/// and is called with the task's cancel handle
///
/// `resume` is a function that is called when the task finishes,
/// to signal to the `block`ing function that it should return
pub fn block_on<Block: FnOnce(T::Cancel), Resume: FnOnce(), T: Task>(
	block: Block, resume: Resume, task: T
) -> T::Output {
	let mut state: BlockState<Resume, T::Output> = BlockState::Pending(resume);

	unsafe {
		let request = Request::new(
			MutPtr::from(&mut state).as_unit().into(),
			block_resume::<Resume, T::Output>
		);

		match task.run(Ptr::from(&request)) {
			Progress::Pending(cancel) => block(cancel),
			Progress::Done(value) => return value
		};
	};

	let BlockState::Done(output) = state else {
		panic!("Blocking function ended before producing a result");
	};

	output
}
