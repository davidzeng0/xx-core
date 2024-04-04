#![allow(clippy::multiple_unsafe_ops_per_block)]

use std::result;

use super::*;

#[derive(Debug)]
pub struct Join<O1, O2>(pub O1, pub O2);

impl<O1, O2, E> Join<result::Result<O1, E>, result::Result<O2, E>> {
	/// Flatten the `Join`, returning the first error it encounters
	pub fn flatten(self) -> result::Result<Join<O1, O2>, E> {
		Ok(Join(self.0?, self.1?))
	}
}

impl<O1, O2> Join<Option<O1>, Option<O2>> {
	/// Flatten the `Join`, returning none if there are any
	pub fn flatten(self) -> Option<Join<O1, O2>> {
		Some(Join(self.0?, self.1?))
	}
}

impl<O1, O2> Join<O1, O2> {
	/// # Safety
	/// branch must be a result of a valid join futures call
	pub unsafe fn from_branch(branch: BranchOutput<O1, O2>) -> Self {
		let BranchOutput(_, a, b) = branch;

		match Join(a, b).flatten() {
			Some(result) => result,

			/* Safety: both tasks must run to completion */
			_ => unsafe { unreachable_unchecked!("Branch failed") }
		}
	}

	/// # Safety
	/// branch must be a result of a valid join call
	pub unsafe fn from_spawn_branch(
		branch: BranchOutput<SpawnResult<O1>, SpawnResult<O2>>
	) -> Self {
		let BranchOutput(_, a, b) = branch;
		let result = Join(a.transpose(), b.transpose()).flatten();

		match runtime::join(result).flatten() {
			Some(result) => result,

			/* Safety: both tasks must run to completion. if one task panicked, this function
			 * would panic */
			_ => unsafe { unreachable_unchecked!("Branch failed") }
		}
	}
}

#[asynchronous]
pub async fn join_future<F1, F2>(future_1: F1, future_2: F2) -> Join<F1::Output, F2::Output>
where
	F1: Future,
	F2: Future
{
	let branch = branch(future_1, future_2, (|_| false, |_| false)).await;

	/* Safety: this is a join future */
	unsafe { Join::from_branch(branch) }
}

/// Joins two tasks A and B and waits
/// for both of them to finish, returning
/// both of their results
///
/// If one task panics, an attempt to cancel the other is made,
/// then the panic resumes on the caller
///
/// # Safety
/// The executor and the created runtime outlive the worker
#[asynchronous]
pub async unsafe fn join<E, T1, T2>(
	runtime: Ptr<E>, task_1: T1, task_2: T2
) -> Join<T1::Output, T2::Output>
where
	E: Environment,
	T1: Task,
	T2: Task
{
	/* Safety: guaranteed by caller */
	let branch = unsafe {
		branch(
			spawn_task_with_env(runtime, task_1),
			spawn_task_with_env(runtime, task_2),
			(
				|result| !matches!(result, Ok(Ok(_))),
				|result| !matches!(result, Ok(Ok(_)))
			)
		)
	}
	.await;

	/* Safety: this is a join */
	unsafe { Join::from_spawn_branch(branch) }
}
