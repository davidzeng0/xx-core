use std::mem::replace;
#[cfg(not(debug_assertions))]
use std::mem::ManuallyDrop;

use super::*;
#[cfg(debug_assertions)]
use crate::macros::unreachable_unchecked;

#[cfg(debug_assertions)]
enum BlockState<Resume, Output> {
	Pending(Resume),
	Done(Output)
}

#[cfg(debug_assertions)]
impl<Resume, Output> BlockState<Resume, Output> {
	const fn pending(resume: Resume) -> Self {
		Self::Pending(resume)
	}

	unsafe fn complete(&mut self, value: Output) -> Resume {
		let resume = replace(self, Self::Done(value));

		match resume {
			Self::Pending(resume) => resume,

			/* Safety: future cannot complete twice */
			Self::Done(_) => unsafe { unreachable_unchecked!("Double complete detected") }
		}
	}

	unsafe fn output(self) -> Output {
		match self {
			Self::Done(output) => output,

			/* Safety: guaranteed by caller */
			Self::Pending(_) => unsafe {
				unreachable_unchecked!("Blocking function ended before producing a result")
			}
		}
	}
}

#[cfg(not(debug_assertions))]
union BlockState<Resume, Output> {
	pending: ManuallyDrop<Resume>,
	done: ManuallyDrop<Output>
}

#[cfg(not(debug_assertions))]
impl<Resume, Output> BlockState<Resume, Output> {
	const fn pending(resume: Resume) -> Self {
		Self { pending: ManuallyDrop::new(resume) }
	}

	unsafe fn complete(&mut self, value: Output) -> Resume {
		let resume = replace(self, Self { done: ManuallyDrop::new(value) });

		/* Safety: guaranteed by caller */
		ManuallyDrop::into_inner(unsafe { resume.pending })
	}

	const unsafe fn output(self) -> Output {
		/* Safety: guaranteed by caller */
		ManuallyDrop::into_inner(unsafe { self.done })
	}
}

unsafe fn block_resume<Resume, Output>(_: ReqPtr<Output>, arg: Ptr<()>, value: Output)
where
	Resume: FnOnce()
{
	/* Safety: arg is valid until resume is called */
	let arg = unsafe { arg.cast::<BlockState<Resume, Output>>().cast_mut().as_mut() };

	/* Safety: guaranteed by caller */
	let resume = unsafe { arg.complete(value) };

	call_no_unwind(resume);
}

struct Waiter<Resume, Output> {
	request: Request<Output>,
	state: BlockState<Resume, Output>
}

impl<Resume: FnOnce(), Output> Waiter<Resume, Output> {
	const fn new(resume: Resume) -> Self {
		/* Safety: block_resume does not unwind */
		let request = unsafe { Request::new(Ptr::null(), block_resume::<Resume, Output>) };

		Self { request, state: BlockState::pending(resume) }
	}

	#[allow(clippy::missing_const_for_fn)]
	unsafe fn output(self) -> Output {
		/* Safety: guaranteed by caller */
		unsafe { self.state.output() }
	}
}

impl<Resume, Output> Pin for Waiter<Resume, Output> {
	unsafe fn pin(&mut self) {
		self.request
			.set_arg(ptr!(&mut self.state).cast_const().cast());
	}
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
/// `resume` must never unwind
pub unsafe fn block_on<Block, Resume, F>(block: Block, resume: Resume, future: F) -> F::Output
where
	Block: FnOnce(F::Cancel),
	Resume: FnOnce(),
	F: Future
{
	let mut waiter = Waiter::new(resume);

	{
		let waiter = waiter.pin_local();

		/* Safety: contract upheld by caller */
		unsafe {
			match future.run(ptr!(&waiter.request)) {
				Progress::Pending(cancel) => block(cancel),
				Progress::Done(value) => return value
			};
		};
	}

	/* Safety: contract upheld by caller */
	unsafe { waiter.output() }
}
