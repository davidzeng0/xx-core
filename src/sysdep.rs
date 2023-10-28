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
