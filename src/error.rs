#![allow(clippy::module_name_repetitions)]

use std::{
	error,
	fmt::{self, Debug, Display, Formatter},
	io, result
};

use crate::os::error::OsError;

pub mod re_exports {
	pub use anyhow;
	pub use thiserror;
}

mod private {
	pub trait Sealed {}
}

pub use io::ErrorKind;

pub use crate::macros::errors;

pub type Result<T> = result::Result<T, Error>;
pub type OsResult<T> = result::Result<T, OsError>;

impl<T> private::Sealed for Result<T> {}

pub trait ErrorContext<T>: private::Sealed {
	fn context<C>(self, context: C) -> Result<T>
	where
		C: Display + Send + Sync + 'static;

	fn with_context<C, F>(self, context: F) -> Result<T>
	where
		C: Display + Send + Sync + 'static,
		F: FnOnce() -> C;
}

impl<T> ErrorContext<T> for Result<T> {
	fn context<C>(self, context: C) -> Self
	where
		C: Display + Send + Sync + 'static
	{
		self.with_context(|| context)
	}

	fn with_context<C, F>(self, context: F) -> Self
	where
		C: Display + Send + Sync + 'static,
		F: FnOnce() -> C
	{
		match self {
			Ok(ok) => Ok(ok),
			Err(err) => Err(err.context(context()))
		}
	}
}

pub trait IntoError: error::Error + Send + Sync + Sized + 'static {
	fn into_err(self) -> Error {
		Error(self.into())
	}
}

pub struct Error(anyhow::Error);

impl Error {
	#[must_use]
	pub fn os_error(&self) -> Option<OsError> {
		self.0.downcast_ref::<Os>().map(|os| os.0)
	}

	#[must_use]
	pub fn kind(&self) -> ErrorKind {
		if let Some(os) = self.os_error() {
			return os.kind();
		}

		if let Some(core) = self.0.downcast_ref::<Core>() {
			return core.kind();
		}

		ErrorKind::Other
	}

	#[must_use]
	pub fn is_interrupted(&self) -> bool {
		self.kind() == ErrorKind::Interrupted
	}

	#[must_use]
	pub fn map<E>(err: E) -> Self
	where
		E: error::Error + Send + Sync + 'static
	{
		Self(err.into())
	}

	pub fn context<C>(self, context: C) -> Self
	where
		C: Display + Send + Sync + 'static
	{
		Self(self.0.context(context))
	}
}

impl<T: PartialEq + IntoError> PartialEq<T> for Error {
	fn eq(&self, other: &T) -> bool {
		self.0
			.downcast_ref::<T>()
			.is_some_and(|inner| inner.eq(other))
	}
}

impl Debug for Error {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		Debug::fmt(&self.0, fmt)
	}
}

impl Display for Error {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		Display::fmt(&self.0, fmt)
	}
}

impl From<anyhow::Error> for Error {
	fn from(value: anyhow::Error) -> Self {
		Self(value)
	}
}

impl From<io::Error> for Error {
	#[allow(clippy::unwrap_used)]
	fn from(value: io::Error) -> Self {
		if let Some(code) = value.raw_os_error() {
			return Os(OsError::from_raw(code)).into();
		}

		if value
			.get_ref()
			.and_then(|err| err.downcast_ref::<Self>())
			.is_some()
		{
			return *value.into_inner().unwrap().downcast().unwrap();
		}

		Self(value.into())
	}
}

impl<T: IntoError> From<T> for Error {
	fn from(value: T) -> Self {
		value.into_err()
	}
}

impl From<Error> for io::Error {
	fn from(value: Error) -> Self {
		Self::new(value.kind(), value)
	}
}

impl error::Error for Error {}

impl From<OsError> for Error {
	fn from(value: OsError) -> Self {
		Os(value).into()
	}
}

#[derive(Clone, Copy)]
pub struct Os(pub OsError);

impl Display for Os {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		write!(fmt, "{} (os error {})", self.0.as_str(), self.0 as i32)
	}
}

impl Debug for Os {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		fmt.debug_struct("Os")
			.field("code", &(self.0 as i32))
			.field("kind", &self.0.kind())
			.field("message", &self.0.as_str())
			.finish()
	}
}

impl IntoError for Os {}

impl error::Error for Os {}

#[errors]
pub enum Core {
	#[error("Entity not found")]
	NotFound,

	#[error("Permission denied")]
	PermissionDenied,

	#[error("{0}")]
	Interrupted(&'static str),

	#[error("Write EOF")]
	WriteZero,

	#[error("Invalid UTF-8 found in stream")]
	InvalidUtf8,

	#[error("Unexpected EOF")]
	UnexpectedEof,

	#[error("Overflow occurred")]
	Overflow,

	#[error("Out of memory")]
	OutOfMemory,

	#[error("Address list empty")]
	NoAddresses,

	#[error("Path string contained a null byte")]
	InvalidCStr,

	#[error("Connect timed out")]
	ConnectTimeout,

	#[error("Endpoint is shutdown")]
	Shutdown,

	#[error("Formatter error")]
	FormatterError,

	#[error("{0}")]
	Pending(&'static str)
}

impl Core {
	#[must_use]
	pub const fn kind(&self) -> ErrorKind {
		match self {
			Self::NotFound => ErrorKind::NotFound,
			Self::PermissionDenied => ErrorKind::PermissionDenied,
			Self::Interrupted(_) => ErrorKind::Interrupted,
			Self::WriteZero => ErrorKind::WriteZero,
			Self::InvalidUtf8 => ErrorKind::InvalidData,
			Self::UnexpectedEof => ErrorKind::UnexpectedEof,
			Self::Overflow => ErrorKind::InvalidInput,
			Self::OutOfMemory => ErrorKind::OutOfMemory,
			Self::NoAddresses => ErrorKind::InvalidInput,
			Self::InvalidCStr => ErrorKind::InvalidData,
			Self::ConnectTimeout => ErrorKind::TimedOut,
			Self::Shutdown => ErrorKind::NotConnected,
			Self::FormatterError => ErrorKind::Other,
			Self::Pending(_) => ErrorKind::Other
		}
	}

	#[must_use]
	pub const fn interrupted() -> Self {
		Self::Interrupted("Interrupted")
	}
}

#[macro_export]
macro_rules! fmt_error {
	($($arg:tt)*) => {
		<$crate::error::Error as ::std::convert::From<
			$crate::error::re_exports::anyhow::Error
		>>::from($crate::error::re_exports::anyhow::anyhow!($($arg)*))
	}
}

pub use fmt_error;
