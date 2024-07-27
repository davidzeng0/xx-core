use crate::error::*;
pub use crate::macros::future;
use crate::pointer::*;
use crate::runtime::call_no_unwind;

mod block_on;

#[doc(hidden)]
pub mod closure;

#[doc(inline)]
pub use block_on::*;

pub type ReqPtr<T> = Ptr<Request<T>>;
pub type Complete<T> = unsafe fn(ReqPtr<T>, Ptr<()>, T);

/// A pointer of a [`Request`] will be passed to a [`Future`] when
/// [`Future::run`] is called
///
/// When the [`Future`] completes, the [`Request`]'s callback will be
/// executed with the result
///
/// The pointer may be used as the key for [`Cancel::run`],
/// which will cancel at least one Future with the same key
///
/// Each request pointer should be unique, as it may be possible that
/// only one future can be started for each request
///
/// The lifetime of the request must last until the callback is executed
pub struct Request<T> {
	/// The user data to be passed back.
	pub arg: Ptr<()>,
	pub callback: Complete<T>
}

impl<T> Clone for Request<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T> Copy for Request<T> {}

/* Safety: request may only be completed once */
unsafe impl<T> Send for Request<T> {}

/* Safety: request may only be completed once */
unsafe impl<T> Sync for Request<T> {}

impl<T> Request<T> {
	pub const fn no_op() -> Self {
		fn no_op<T>(_: ReqPtr<T>, _: Ptr<()>, _: T) {}

		/* Safety: no_op does not unwind */
		unsafe { Self::new(Ptr::null(), no_op) }
	}

	/// # Safety
	/// `callback` must not unwind, and its safety requirements must be
	/// identical to `Request::complete`
	pub const unsafe fn new(arg: Ptr<()>, callback: Complete<T>) -> Self {
		Self { arg, callback }
	}

	pub fn set_arg(&mut self, arg: Ptr<()>) {
		self.arg = arg;
	}

	/// # Safety
	/// must not call after future already completed, and must not call within
	/// `Future::run`
	///
	/// Whether or not it is safe to complete a `Future` from a different
	/// thread is defined by the `Future` implementation
	pub unsafe fn complete(request: Ptr<Self>, value: T) {
		/* Safety: guaranteed by caller and Future's contract */
		let Self { arg, callback } = unsafe { ptr!(*request) };

		/* Safety: guaranteed by caller */
		call_no_unwind(|| unsafe { (callback)(request, arg, value) });
	}
}

/// A cancel token, allowing the user to cancel a running future
///
/// # Safety
/// If `run` panics, the callback must not be called from within, and the future
/// is assumed to still be in progress
pub unsafe trait Cancel {
	/// Cancelling is on a best-effort basis
	///
	/// If the cancellation fails, the user should
	/// ignore the error and hope that the future
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
	/// to be called before releasing the [`Request`] or any data passed into
	/// the future
	///
	/// Cancel implementers must not expect captured references to
	/// live until the (possibly asynchronous) cancel finishes, only until the
	/// future callback is called
	///
	/// It is possible that the callback is
	/// immediately executed in the call to cancel
	///
	/// # Safety
	/// The future must be in progress and this function cannot be called more
	/// than once. The caller must be prepared for the callback to be executed
	/// from within this call
	unsafe fn run(self) -> Result<()>;
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

/// # Safety
/// Must not use any references once the callback is called
/// If `run` panics, the future must not be in progress, and the callback must
/// not be called
#[must_use = "Future does nothing until `Future::run` is called"]
pub unsafe trait Future {
	type Output;
	type Cancel: Cancel;

	/// Run the future
	///
	/// The user is responsible for ensuring all pointers/references passed
	/// to the future implementer stays valid until the callback is called
	///
	/// If the future can be constructed with raw pointers and integers in safe
	/// code, the constructor must be marked as unsafe, as safe blocking
	/// functions cannot guarantee all lifetimes are captured and life for the
	/// blocking duration
	///
	/// # Safety
	/// All pointers must live until the callback is called
	unsafe fn run(self, request: ReqPtr<Self::Output>) -> Progress<Self::Output, Self::Cancel>;
}
