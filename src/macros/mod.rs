pub use xx_core_macros::{
	asynchronous, compact_error, duration, future, syscall_define, syscall_impl, wrapper_functions
};

mod pointer;
pub use pointer::*;
mod branch;
pub use branch::*;

#[macro_export]
macro_rules! macro_each {
	($macro:ident, $item:tt $($each:tt)*) => {
		$macro!($item);

		$crate::macros::macro_each!($macro $($each)*);
	};

	($macro:ident) => {}
}

pub use macro_each;

#[macro_export]
macro_rules! import_sysdeps {
	() => {
		#[cfg(target_arch = "aarch64")]
		mod arm64;
		#[cfg(target_arch = "x86_64")]
		mod x64;

		mod platform {
			#[cfg(target_arch = "aarch64")]
			#[allow(unused_imports)]
			pub use super::arm64::*;
			#[cfg(target_arch = "x86_64")]
			#[allow(unused_imports)]
			pub use super::x64::*;
		}

		#[allow(unused_imports)]
		use platform::*;
	};
}

pub use import_sysdeps;
