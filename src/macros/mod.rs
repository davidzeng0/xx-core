pub use xx_core_macros::*;

pub mod pointer;
pub mod runtime;
pub mod traits;
pub use pointer::*;
pub use runtime::*;
pub use traits::*;

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
		#[cfg(any(target_arch = "aarch64", doc))]
		mod arm64;
		#[cfg(any(target_arch = "x86_64", doc))]
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

#[macro_export]
macro_rules! require_unsafe {
	() => {{
		const unsafe fn require_unsafe() {}

		require_unsafe();
	}};
}

pub use require_unsafe;
