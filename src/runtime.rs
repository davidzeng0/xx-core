use std::{fmt::Arguments, panic::*};

use crate::log::*;

pub type MaybePanic<T> = std::thread::Result<T>;

#[track_caller]
pub fn panic_nounwind(fmt: Arguments<'_>) -> ! {
	print_panic(None, fmt);
	print_fatal(format_args!("Non unwinding panic, aborting"));

	std::process::abort();
}

/// # Panics
/// resumes the panic if `result` is an `Err`
pub fn join<T>(result: MaybePanic<T>) -> T {
	match result {
		Ok(ok) => ok,
		Err(err) => resume_unwind(err)
	}
}

#[inline(always)]
pub fn call_non_panicking<F, Output>(func: F) -> Output
where
	F: FnOnce() -> Output
{
	#[cfg(debug_assertions)]
	match catch_unwind(AssertUnwindSafe(func)) {
		Ok(ok) => ok,
		Err(_) => crate::macros::panic_nounwind!("Function that must never panic panicked")
	}

	#[cfg(not(debug_assertions))]
	func()
}
