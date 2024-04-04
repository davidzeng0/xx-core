#[macro_export]
macro_rules! container_of {
	($ptr:expr, $type:ty : $field:ident) => {
		($crate::pointer::Pointer::cast::<u8>($ptr) - ::std::mem::offset_of!($type, $field))
			.cast::<$type>()
	};
}

pub use container_of;

#[macro_export]
macro_rules! ptr {
	(*$ptr:expr) => {
		(*$crate::pointer::internal::AsPointer::as_pointer(&$ptr))
	};

	(&$value:expr) => {
		$crate::pointer::Pointer::from(::std::ptr::addr_of!($value))
	};

	(&mut $value:expr) => {
		$crate::pointer::Pointer::from(::std::ptr::addr_of_mut!($value))
	};

	(&$ptr:expr => $($expr:tt)*) => {
		$crate::macros::ptr!(
			&$crate::macros::ptr!($ptr => $($expr)*)
		)
	};

	(&mut $ptr:expr => $($expr:tt)*) => {
		$crate::macros::ptr!(
			&mut $crate::macros::ptr!($ptr => $($expr)*)
		)
	};

	($ptr:expr => [$index:expr] $($expr:tt)*) => {
		$crate::macros::ptr!(*$ptr)[$index]$($expr)*
	};

	($ptr:expr => $($expr:tt)*) => {
		$crate::macros::ptr!(*$ptr).$($expr)*
	};

	($ref:expr) => {
		$crate::pointer::Pointer::from($ref)
	};
}

pub use ptr;
