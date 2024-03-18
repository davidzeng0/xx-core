#[macro_export]
macro_rules! abort {
	($($arg: tt)*) => {
		$crate::runtime::panic_nounwind(::std::format_args!($($arg)*))
	}
}

pub use abort;

#[macro_export]
macro_rules! unreachable_unchecked {
	($($arg: tt)*) => {{
		$crate::macros::require_unsafe!();

		#[cfg(debug_assertions)]
		$crate::runtime::panic_nounwind(::std::format_args!(
			"Entered unreachable code: {}",
			::std::format_args!($($arg)*)
		));

		#[cfg(not(debug_assertions))]
		$crate::opt::hint::unreachable_unchecked();
	}}
}

pub use unreachable_unchecked;

#[macro_export]
macro_rules! unwrap_panic {
	($result:expr) => {
		match $result {
			Ok(ok) => ok,
			Err(err) => ::std::panic::resume_unwind(err)
		}
	};
}

pub use unwrap_panic;
