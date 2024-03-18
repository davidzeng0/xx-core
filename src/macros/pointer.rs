#[macro_export]
macro_rules! require_unsafe {
	() => {
		({
			const unsafe fn require_unsafe() {}

			require_unsafe();
		})
	};
}

pub use require_unsafe;

#[macro_export]
macro_rules! offset_of {
	($type:ty, $field:ident) => {{
		/* Safety: just pointer arithmetic */
		#[allow(unused_unsafe, clippy::undocumented_unsafe_blocks)]
		#[allow(clippy::multiple_unsafe_ops_per_block)]
		unsafe {
			let invalid: *const $type = ::std::ptr::null();
			let field = ::std::ptr::addr_of!((*invalid).$field);

			$crate::pointer::Ptr::from(field).int_addr()
		}
	}};
}

pub use offset_of;

#[macro_export]
macro_rules! container_of {
	($ptr:expr, $type:ty : $field:ident) => {
		($crate::pointer::Pointer::cast::<u8>($ptr) - $crate::macros::offset_of!($type, $field))
			.cast::<$type>()
	};
}

pub use container_of;
