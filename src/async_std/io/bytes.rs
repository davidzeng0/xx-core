pub trait BytesEncoding<const N: usize> {
	fn to_bytes_le(self) -> [u8; N];
	fn to_bytes_be(self) -> [u8; N];

	fn from_bytes_le(bytes: [u8; N]) -> Self;
	fn from_bytes_be(bytes: [u8; N]) -> Self;
}

macro_rules! impl_bytes_type_bits {
	($type: ty, $bits: literal) => {
		impl BytesEncoding<{ $bits as usize / 8 }> for $type {
			#[inline(always)]
			fn to_bytes_le(self) -> [u8; $bits as usize / 8] {
				self.to_le_bytes()
			}

			#[inline(always)]
			fn to_bytes_be(self) -> [u8; $bits as usize / 8] {
				self.to_be_bytes()
			}

			#[inline(always)]
			fn from_bytes_le(bytes: [u8; $bits as usize / 8]) -> Self {
				Self::from_le_bytes(bytes)
			}

			#[inline(always)]
			fn from_bytes_be(bytes: [u8; $bits as usize / 8]) -> Self {
				Self::from_be_bytes(bytes)
			}
		}
	};
}

/* usize and isize omitted intentionally */
impl_bytes_type_bits!(i8, 8);
impl_bytes_type_bits!(u8, 8);
impl_bytes_type_bits!(i16, 16);
impl_bytes_type_bits!(u16, 16);
impl_bytes_type_bits!(i32, 32);
impl_bytes_type_bits!(u32, 32);
impl_bytes_type_bits!(i64, 64);
impl_bytes_type_bits!(u64, 64);
impl_bytes_type_bits!(i128, 128);
impl_bytes_type_bits!(u128, 128);
impl_bytes_type_bits!(f32, 32);
impl_bytes_type_bits!(f64, 64);
