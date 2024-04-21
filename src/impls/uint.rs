use crate::macros::macro_each;

pub trait UIntExtensions {
	type Signed;

	fn overflowing_signed_difference(self, rhs: Self) -> (Self::Signed, bool);
	fn saturating_signed_difference(self, rhs: Self) -> Self::Signed;
}

macro_rules! uint_impl {
	(($type:ty, $signed:ty)) => {
		impl UIntExtensions for $type {
			type Signed = $signed;

			fn overflowing_signed_difference(self, rhs: Self) -> ($signed, bool) {
				#[allow(clippy::cast_possible_wrap)]
				let res = self.wrapping_sub(rhs) as $signed;
				let overflow = (self >= rhs) == (res < 0);

				(res, overflow)
			}

			fn saturating_signed_difference(self, rhs: Self) -> $signed {
				let (res, overflow) = self.overflowing_signed_difference(rhs);

				if !overflow {
					res
				} else if res < 0 {
					<$signed>::MAX
				} else {
					<$signed>::MIN
				}
			}
		}
	};
}

macro_each!(
	uint_impl,
	(u8, i8),
	(u16, i16),
	(u32, i32),
	(u64, i64),
	(u128, i128),
	(usize, isize)
);
