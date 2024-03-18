use std::fmt::Arguments;

use crate::log::{print_fatal, print_panic};

pub fn panic_nounwind(fmt: Arguments<'_>) -> ! {
	print_panic(None, fmt);
	print_fatal(format_args!("Non unwinding panic, aborting"));

	std::process::abort();
}
