use super::*;

/// Splits a stream into a read half and a write half.
pub trait Split: Read + Write {
	type Reader: Read;
	type Writer: Write;

	fn split(&mut self) -> (Self::Reader, Self::Writer);
}
