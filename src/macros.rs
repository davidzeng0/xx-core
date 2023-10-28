pub use xx_core_macros::*;

#[macro_export]
macro_rules! offset_of {
	($type: ty, $field: ident) => {
		(&unsafe { &*(0usize as *const $type) }.$field) as *const _ as usize
	};
}

#[macro_export]
macro_rules! container_of {
	($val: expr, $type: ty, $field: ident) => {
		&mut *(($val as *const _ as *const ())
			.cast::<u8>()
			.wrapping_sub($crate::offset_of!($type, $field))
			.cast::<$type>() as *mut $type)
	};
}

pub mod closure {
	pub mod lifetime {
		pub trait Captures<'__> {}

		impl<T: ?Sized> Captures<'_> for T {}
	}
}
