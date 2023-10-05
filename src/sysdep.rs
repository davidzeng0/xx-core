macro_rules! import_sysdeps {
	() => {
		#[cfg(target_arch = "arm64")]
		mod arm64;
		#[cfg(target_arch = "x86_64")]
		mod x64;

		mod platform {
			#[cfg(target_arch = "arm64")]
			pub use super::arm64::*;
			#[cfg(target_arch = "x86_64")]
			pub use super::x64::*;
		}

		#[allow(unused_imports)]
		use platform::*;
	};
}

pub(crate) use import_sysdeps;
