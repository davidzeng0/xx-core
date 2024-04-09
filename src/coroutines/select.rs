#![allow(clippy::multiple_unsafe_ops_per_block, clippy::module_name_repetitions)]

use std::result;

use super::*;

#[derive(Debug)]
pub enum Select<O1, O2> {
	First(O1, Option<O2>),
	Second(O2, Option<O1>)
}

impl<O1, O2> Select<O1, O2> {
	pub fn first(self) -> Option<O1> {
		match self {
			Self::First(result, _) => Some(result),
			Self::Second(..) => None
		}
	}

	pub fn second(self) -> Option<O2> {
		match self {
			Self::First(..) => None,
			Self::Second(result, _) => Some(result)
		}
	}
}

impl<O1, O2, E> Select<result::Result<O1, E>, result::Result<O2, E>> {
	/// Flatten the `Select`, returning the first error it encounters
	pub fn flatten(self) -> result::Result<Select<O1, O2>, E> {
		Ok(match self {
			Self::First(a, b) => Select::First(a?, b.transpose()?),
			Self::Second(a, b) => Select::Second(a?, b.transpose()?)
		})
	}
}

impl<O1, O2> Select<Option<O1>, Option<O2>> {
	/// Flatten the `Select`, returning none if there are any
	pub fn flatten(self) -> Option<Select<O1, O2>> {
		Some(match self {
			Self::First(a, b) => Select::First(a?, b.flatten()),
			Self::Second(a, b) => Select::Second(a?, b.flatten())
		})
	}
}

impl<O1, O2> Select<O1, O2> {
	/// # Safety
	/// branch must be a result of a valid select
	pub unsafe fn from_branch(branch: BranchOutput<O1, O2>) -> Self {
		let BranchOutput(is_first, a, b) = branch;

		match (is_first, a.map(runtime::join), b.map(runtime::join)) {
			(true, Some(a), b) => Self::First(a, b),
			(false, a, Some(b)) => Self::Second(b, a),
			/* Safety: at least one task must be completed */
			_ => unsafe { unreachable_unchecked!("Branch failed") }
		}
	}
}

#[asynchronous]
pub async fn select_future<F1, F2>(future_1: F1, future_2: F2) -> Select<F1::Output, F2::Output>
where
	F1: Future,
	F2: Future
{
	/* Safety: should_cancel does not panic */
	let result = unsafe { branch(future_1, future_2, (|_| true, |_| true)).await };

	/* Safety: this is a select */
	unsafe { Select::from_branch(result) }
}

/// Races two tasks A and B and waits
/// for one of them to finish and cancelling the other
///
/// Returns `Select::First` if the first task completed first
/// or `Select::Second` if the second task completed first
///
/// If both tasks are started successfully, the second parameter
/// in `Select` will contain the result from the second task
///
/// If one of the task panics, the panic is resumed on the caller
///
/// # Safety
/// The executor and the created runtime outlive the worker
#[asynchronous]
pub async unsafe fn select<E, T1, T2>(
	runtime: &E, task_1: T1, task_2: T2
) -> Select<T1::Output, T2::Output>
where
	E: Environment,
	T1: Task,
	T2: Task
{
	/* Safety: guaranteed by caller */
	let result = unsafe {
		select_future(
			spawn_task_with_env(runtime, task_1),
			spawn_task_with_env(runtime, task_2)
		)
		.await
	};

	runtime::join(result.flatten())
}
