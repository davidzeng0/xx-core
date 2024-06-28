#![allow(clippy::module_name_repetitions)]

use crate::macros::{panic_nounwind, seal_trait, unreachable_unchecked};

seal_trait!();

pub trait OptionExt<T>: Sealed + Sized {
	fn expect_nounwind(self, msg: &str) -> T;

	/// # Safety
	/// `self` must be `Some`
	unsafe fn expect_unchecked(self, msg: &str) -> T;
}

impl<T> Sealed for Option<T> {}

impl<T> OptionExt<T> for Option<T> {
	#[track_caller]
	fn expect_nounwind(self, msg: &str) -> T {
		match self {
			Some(val) => val,
			None => panic_nounwind!("{}", msg)
		}
	}

	#[track_caller]
	unsafe fn expect_unchecked(self, msg: &str) -> T {
		match self {
			Some(val) => val,

			/* Safety: guaranteed by caller */
			None => unsafe { unreachable_unchecked!("{}", msg) }
		}
	}
}
