#![allow(clippy::module_name_repetitions)]

pub trait ToBytes<const N: usize> {
	fn to_bytes(self) -> [u8; N];
}

pub trait FromBytes<const N: usize> {
	fn from_bytes(bytes: [u8; N]) -> Self;
}

impl<const N: usize> ToBytes<N> for [u8; N] {
	fn to_bytes(self) -> [u8; N] {
		self
	}
}

impl<const N: usize> FromBytes<N> for [u8; N] {
	fn from_bytes(bytes: [u8; N]) -> Self {
		bytes
	}
}
