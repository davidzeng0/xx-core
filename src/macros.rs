pub use xx_core_macros::*;

pub mod closure {
	pub trait Captures<'__> {}

	impl<T: ?Sized> Captures<'_> for T {}
}
