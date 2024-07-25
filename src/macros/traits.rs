use super::*;

#[macro_export]
macro_rules! sealed_trait {
	($($name:ident)?) => {
		$crate::paste::paste! {
			#[allow(non_snake_case)]
			mod [< __private_seal_ $($name)? >] {
				pub trait [< $($name)? Sealed >] {}
			}

			use [< __private_seal_ $($name)? >]::[< $($name)? Sealed >];
		}
	};

	(for $trait:ident) => {
		$crate::paste::paste! {
			#[allow(non_snake_case)]
			mod [< __private_seal_ $trait >] {
				pub trait [< $trait Sealed >]: super::$trait {}

				impl<T: super::$trait> [< $trait Sealed >] for T {}
			}

			use [< __private_seal_ $trait >]::[< $trait Sealed >];
		}
	};

	(($($tokens:tt)*)) => {
		$crate::macros::sealed_trait!($($tokens)*);
	}
}

pub use sealed_trait;
