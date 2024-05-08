use super::*;

/// Splits a stream into a read half and a write half.
pub trait Split: Read + Write {
	type Reader: Read;
	type Writer: Write;

	fn split(&mut self) -> (Self::Reader, Self::Writer) {
		self.try_split().unwrap()
	}

	fn try_split(&mut self) -> Result<(Self::Reader, Self::Writer)>;
}
