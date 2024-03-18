#![allow(clippy::multiple_unsafe_ops_per_block)]

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

#[asynchronous]
pub async fn select_future<F1, F2>(future_1: F1, future_2: F2) -> Select<F1::Output, F2::Output>
where
	F1: Future,
	F2: Future
{
	let mut data = Branch::new(future_1, future_2, (|_| true, |_| true));

	/* Safety: data is pinned */
	let BranchOutput(is_first, a, b) = block_on(unsafe { data.pin_local().run() }).await;

	match (is_first, a, b) {
		(true, Some(a), b) => Select::First(a, b),
		(false, a, Some(b)) => Select::Second(b, a),
		/* Safety: at least one task must be completed */
		_ => unsafe { unreachable_unchecked!("Branch failed") }
	}
}

/// Races two tasks A and B and waits
/// for one of them to finish and cancelling the other
///
/// Returns `Select::First` if the first task completed first
/// or `Select::Second` if the second task completed first
///
/// If both tasks are started successfully, the second parameter
/// in `Select` will contain the result from the second task
#[asynchronous]
pub async fn select<E, T1, T2>(
	runtime: Ptr<E>, task_1: T1, task_2: T2
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

	unwrap_panic!(result.flatten())
}
