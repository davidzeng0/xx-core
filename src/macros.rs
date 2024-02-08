pub(crate) use xx_core_macros::syscall_impl;
pub use xx_core_macros::{asynchronous, compact_error, duration, future, wrapper_functions};

#[macro_export]
macro_rules! offset_of {
	($type: ty, $field: ident) => {{
		unsafe fn require_unsafe() {}

		require_unsafe();

		let field = &$crate::pointer::Ptr::<$type>::null().$field;

		$crate::pointer::Ptr::from(field).int_addr()
	}};
}

pub use offset_of;

#[macro_export]
macro_rules! container_of {
	($ptr: expr, $type: ty, $field: ident) => {
		$ptr.cast::<u8>()
			.sub($crate::offset_of!($type, $field))
			.cast::<$type>()
			.cast_mut()
	};
}

pub use container_of;

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

pub(crate) use import_sysdeps;
