use super::*;

#[macro_export]
macro_rules! seal_trait {
	($trait:ident) => {
		$crate::paste::paste! {
			#[allow(non_snake_case)]
			mod [< __private_seal_ $trait >] {
				pub trait [< $trait Sealed >]: super::$trait {}

				impl<T: super::$trait> [< $trait Sealed >] for T {}
			}

			use [< __private_seal_ $trait >]::[< $trait Sealed >];
		}
	};
}

pub use seal_trait;
