use std::mem::replace;

use super::*;
use crate::{macros::unreachable_unchecked, runtime::call_non_panicking};

enum BlockState<Resume, Output> {
	Pending(Resume),
	Done(Output)
}

unsafe fn block_resume<Resume, Output>(_: ReqPtr<Output>, arg: Ptr<()>, value: Output)
where
	Resume: FnOnce()
{
	/* Safety: arg is valid until resume is called */
	let arg = unsafe { arg.cast::<BlockState<Resume, Output>>().cast_mut().as_mut() };
	let resume = replace(arg, BlockState::Done(value));

	let BlockState::Pending(resume) = resume else {
		/* Safety: future cannot complete twice */
		unsafe { unreachable_unchecked!("Double complete detected") };
	};

	call_non_panicking(resume);
}

/// Block on a future
///
/// `block` is a function that doesn't return until the future finishes,
/// and is called with the future's cancel handle
///
/// `resume` is a function that is called when the future finishes,
/// to signal to the `block`ing function that it should return
///
/// # Safety
/// `block` must block until the future finishes
/// `resume` must never panic
pub unsafe fn block_on<Block, Resume, F>(block: Block, resume: Resume, future: F) -> F::Output
where
	Block: FnOnce(F::Cancel),
	Resume: FnOnce(),
	F: Future
{
	let mut state: BlockState<Resume, F::Output> = BlockState::Pending(resume);

	/* Safety: block_resume does not panic */
	let request = unsafe {
		Request::new(
			ptr!(&mut state).cast_const().cast(),
			block_resume::<Resume, F::Output>
		)
	};

	/* Safety: contract upheld by caller */
	unsafe {
		match future.run(ptr!(&request)) {
			Progress::Pending(cancel) => block(cancel),
			Progress::Done(value) => return value
		};
	};

	let BlockState::Done(output) = state else {
		/* Safety: guaranteed by caller */
		unsafe { unreachable_unchecked!("Blocking function ended before producing a result") };
	};

	output
}
