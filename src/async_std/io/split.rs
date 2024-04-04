use super::*;

/// Splits a stream into a read half and a write half.
pub trait Split: Read + Write {
	type Reader: Read;
	type Writer: Write;

	fn split(&mut self) -> (ReadRef<'_, Self::Reader>, WriteRef<'_, Self::Writer>);
}
