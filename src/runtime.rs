use std::{fmt::Arguments, panic::resume_unwind};

use crate::log::{print_fatal, print_panic};

pub type PanickingResult<T> = std::thread::Result<T>;

pub fn panic_nounwind(fmt: Arguments<'_>) -> ! {
	print_panic(None, fmt);
	print_fatal(format_args!("Non unwinding panic, aborting"));

	std::process::abort();
}

/// # Panics
/// resumes the panic if `result` is an `Err`
pub fn join<T>(result: PanickingResult<T>) -> T {
	match result {
		Ok(ok) => ok,
		Err(err) => resume_unwind(err)
	}
}
