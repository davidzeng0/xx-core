use std::io::Result;

pub use xx_core_macros::sync_task;

use crate::pointer::ConstPtr;
pub mod block_on;
pub mod closure;
pub mod env;

pub type RequestPtr<T> = ConstPtr<Request<T>>;

/// A pointer of a [`Request`] will be passed to a [`Task`] when [`Task::run`]
/// is called
///
/// When the [`Task`] completes, the [`Request`]'s callback will be
/// executed with the result
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
	pub arg: *const (),
	pub callback: fn(ConstPtr<Self>, *const (), T)
}

impl<T> Request<T> {
	pub const unsafe fn new(arg: *const (), callback: fn(ConstPtr<Self>, *const (), T)) -> Self {
		Self { arg, callback }
	}

	pub fn set_arg(&mut self, arg: *const ()) {
		self.arg = arg;
	}

	#[inline(always)]
	pub fn complete(request: ConstPtr<Self>, value: T) {
		(request.callback)(request, request.arg, value);
	}
}

/// A cancel token, allowing the user to cancel a running task
pub unsafe trait Cancel {
	/// Cancelling is on a best-effort basis
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
	/// because cancellations may be asynchronous
	///
	/// After cancelling, you must wait for the callback
	/// to be called before releasing the [`Request`]
	///
	/// It is possible that the callback is
	/// immediately executed in the call to cancel
	unsafe fn run(self) -> Result<()>;
}

pub struct NoOpCancel;

unsafe impl Cancel for NoOpCancel {
	#[inline(always)]
	unsafe fn run(self) -> Result<()> {
		Ok(())
	}
}

#[must_use]
pub enum Progress<Output, C: Cancel> {
	/// The operation is pending
	/// The callback on the request will be called when it is complete
	Pending(C),

	/// The operation completed synchronously
	/// The callback on the request will not be called
	Done(Output)
}

pub unsafe trait Task<Output, C: Cancel = NoOpCancel> {
	/// Run the task
	///
	/// The user is responsible for ensuring any pointers/references passed
	/// to the task stays alive until the callback is called.
	///
	/// Which pointers need to stay valid will depend on the implementation
	unsafe fn run(self, request: RequestPtr<Output>) -> Progress<Output, C>;
}
