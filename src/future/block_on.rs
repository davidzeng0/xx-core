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
		#[cfg(debug_assertions)]
		panic!("Double resume detected");
		#[cfg(not(debug_assertions))]
		unsafe {
			std::hint::unreachable_unchecked()
		};
	};

	resume();
}

/// Block on a future
///
/// `block` is a function that doesn't return until the task finishes,
/// and is called with the task's cancel handle
///
/// `resume` is a function that is called when the task finishes,
/// to signal to the `block`ing function that it should return
pub unsafe fn block_on<Block: FnOnce(T::Cancel), Resume: FnOnce(), T: Future>(
	block: Block, resume: Resume, task: T
) -> T::Output {
	let mut state: BlockState<Resume, T::Output> = BlockState::Pending(resume);

	let request = Request::new(
		MutPtr::from(&mut state).as_unit().into(),
		block_resume::<Resume, T::Output>
	);

	/* Safety: contract upheld by caller */
	unsafe {
		match task.run(Ptr::from(&request)) {
			Progress::Pending(cancel) => block(cancel),
			Progress::Done(value) => return value
		};
	};

	let BlockState::Done(output) = state else {
		#[cfg(debug_assertions)]
		panic!("Blocking function ended before producing a result");
		#[cfg(not(debug_assertions))]
		unsafe {
			std::hint::unreachable_unchecked()
		};
	};

	output
}
