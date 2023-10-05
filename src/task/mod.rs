use std::io::Result;
pub use xx_core_macros::sync_task;
pub mod closure;
pub mod env;

/// A pointer of a [`Request`] will be passed to a [`Task`] when [`Task::run`] is called
/// When the [`Task`] completes, the [`Request`]'s callback will be executed with the result
///
/// The pointer will be used as the key for [`Cancel::run`],
/// which will cancel atleast one Task with the same key
///
/// Each request pointer should be unique, as it may be possible that
/// only one task can be queued for each request
///
/// The lifetime of the request must last until the callback is executed
pub struct Request<T> {
	/// The user data to be passed back.
	arg: *const (),
	callback: fn(*const (), T)
}

impl<T> Request<T> {
	pub unsafe fn new(arg: *const (), callback: fn(*const (), T)) -> Request<T> {
		Request { arg, callback }
	}

	pub fn complete(handle: *const Request<T>, value: T) {
		let handle = unsafe { &*handle };

		(handle.callback)(handle.arg, value);
	}
}

/// A cancel token, allowing the user to cancel a running task
pub unsafe trait Cancel {
	/// Attempt to cancel the task
	///
	/// If the cancellation fails, the user should
	/// ignore the error and pray that the task
	/// completes in a reasonable amount of time
	///
	/// Unless running critically low on memory,
	/// or some other extreme edge case, cancellations
	/// should always succeed
	///
	/// Even if the cancel operation returns an Ok(),
	/// that does not necessarily mean a cancel was successful,
	/// for reasons such as task already completed or out of memory
	///
	/// It is not safe to release the [`Request`] associated
	/// with the task until the callback is ran, even if
	/// an attempted cancellation is in progress
	unsafe fn run(self) -> Result<()>;
}

pub struct NoOpCancel;

unsafe impl Cancel for NoOpCancel {
	unsafe fn run(self) -> Result<()> {
		Ok(())
	}
}

pub enum Progress<Output, C: Cancel> {
	Pending(C),
	Done(Output)
}

pub unsafe trait Task<Output, C: Cancel = NoOpCancel> {
	unsafe fn run(self, request: *const Request<Output>) -> Progress<Output, C>;
}
