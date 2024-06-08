#[macro_export]
macro_rules! container_of {
	($ptr:expr, $type:ty => $field:ident) => {
		$crate::pointer::Pointer::cast::<u8>($ptr)
			.sub(::std::mem::offset_of!($type, $field))
			.cast::<$type>()
	};
}

pub use container_of;

#[macro_export]
macro_rules! ptr {
	(*$ptr:expr) => {
		*$crate::pointer::internal::AsPointer::as_pointer(&$ptr)
	};

	(&$value:expr) => {
		$crate::pointer::Pointer::from(::std::ptr::addr_of!($value))
	};

	(&mut $value:expr) => {
		$crate::pointer::Pointer::from(::std::ptr::addr_of_mut!($value))
	};

	(!null &$value:expr) => {
		({
			const fn as_non_null<T>(ptr: $crate::pointer::Ptr<T>) -> $crate::pointer::NonNull<T> {
				/* Safety: reference of a value is always non null */
				unsafe { $crate::pointer::NonNull::new_unchecked(ptr) }
			}

			as_non_null::<_>
		})($crate::macros::ptr!(&$value))
	};

	(!null &mut $value:expr) => {
		({
			const fn as_non_null<T>(ptr: $crate::pointer::MutPtr<T>) -> $crate::pointer::MutNonNull<T> {
				/* Safety: reference of a value is always non null */
				unsafe { $crate::pointer::MutNonNull::new_unchecked(ptr) }
			}

			as_non_null::<_>
		})($crate::macros::ptr!(&mut $value))
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

	(!null &$ptr:expr => $($expr:tt)*) => {
		$crate::macros::ptr!(
			!null &$crate::macros::ptr!($ptr => $($expr)*)
		)
	};

	(!null &mut $ptr:expr => $($expr:tt)*) => {
		$crate::macros::ptr!(
			!null &mut $crate::macros::ptr!($ptr => $($expr)*)
		)
	};

	($ptr:expr => [$index:expr] $($expr:tt)*) => {
		$crate::macros::ptr!(*
			$crate::pointer::internal::PointerIndex::index($ptr, $index)
		) $($expr)*
	};

	($ptr:expr => $($expr:tt)*) => {
		$crate::macros::ptr!(*$ptr).$($expr)*
	};

	($ref:expr) => {
		$crate::pointer::Pointer::from($ref)
	};

	(!null $ref:expr) => {
		$crate::pointer::NonNullPtr::from($ref)
	};
}

pub use ptr;
