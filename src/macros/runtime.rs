#[macro_export]
macro_rules! panic_nounwind {
	($($arg: tt)*) => {
		$crate::runtime::panic_nounwind(::std::format_args!($($arg)*))
	}
}

pub use panic_nounwind;

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
