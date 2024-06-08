use crate::macros::macro_each;

pub trait UIntExtensions: Sized {
	type Signed;

	#[must_use = "This returns the result of the operation without modifying the original"]
	fn overflowing_signed_diff(self, rhs: Self) -> (Self::Signed, bool);

	#[must_use = "This returns the result of the operation without modifying the original"]
	fn saturating_signed_diff(self, rhs: Self) -> Self::Signed;

	#[must_use = "This returns the result of the operation without modifying the original"]
	fn checked_signed_diff(self, rhs: Self) -> Option<Self::Signed>;

	#[must_use = "This returns the result of the operation without modifying the original"]
	fn overflowing_sub_signed(self, rhs: Self::Signed) -> (Self, bool);

	#[must_use = "This returns the result of the operation without modifying the original"]
	fn saturating_sub_signed(self, rhs: Self::Signed) -> Self;

	#[must_use = "This returns the result of the operation without modifying the original"]
	fn checked_sub_signed(self, rhs: Self::Signed) -> Option<Self>;

	#[must_use = "This returns the result of the operation without modifying the original"]
	fn wrapping_sub_signed(self, rhs: Self::Signed) -> Self;
}

macro_rules! uint_impl {
	(($type:ty, $signed:ty)) => {
		#[allow(clippy::cast_sign_loss)]
		impl UIntExtensions for $type {
			type Signed = $signed;

			fn overflowing_signed_diff(self, rhs: Self) -> ($signed, bool) {
				#[allow(clippy::cast_possible_wrap)]
				let res = self.wrapping_sub(rhs) as $signed;
				let overflow = (self >= rhs) == (res < 0);

				(res, overflow)
			}

			fn saturating_signed_diff(self, rhs: Self) -> $signed {
				let (res, overflow) = self.overflowing_signed_diff(rhs);

				if !overflow {
					res
				} else if res < 0 {
					<$signed>::MAX
				} else {
					<$signed>::MIN
				}
			}

			fn checked_signed_diff(self, rhs: Self) -> Option<$signed> {
				let (res, overflow) = self.overflowing_signed_diff(rhs);

				if !overflow {
					Some(res)
				} else {
					None
				}
			}

			fn overflowing_sub_signed(self, rhs: $signed) -> (Self, bool) {
				let (res, overflow) = self.overflowing_sub(rhs as Self);

				(res, overflow ^ (rhs < 0))
			}

			fn saturating_sub_signed(self, rhs: $signed) -> Self {
				let (res, overflow) = self.overflowing_sub_signed(rhs);

				if !overflow {
					res
				} else if rhs < 0 {
					Self::MAX
				} else {
					0
				}
			}

			fn checked_sub_signed(self, rhs: $signed) -> Option<Self> {
				let (res, overflow) = self.overflowing_sub_signed(rhs);

				if !overflow {
					Some(res)
				} else {
					None
				}
			}

			fn wrapping_sub_signed(self, rhs: $signed) -> Self {
				self.wrapping_sub(rhs as Self)
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
