use core::fmt;
use std::{
	error,
	fmt::{Debug, Display, Formatter},
	io, result
};

use crate::{os::error::ErrorCodes, pointer::ConstPtr};

pub type Result<T> = result::Result<T, Error>;
pub use io::ErrorKind;

pub enum ErrorMessage {
	Static(&'static str),
	Owned(String)
}

impl AsRef<str> for ErrorMessage {
	fn as_ref(&self) -> &str {
		match self {
			Self::Static(val) => *val,
			Self::Owned(val) => val
		}
	}
}

impl From<&'static str> for ErrorMessage {
	fn from(value: &'static str) -> Self {
		Self::Static(value)
	}
}

impl From<String> for ErrorMessage {
	fn from(value: String) -> Self {
		Self::Owned(value)
	}
}

pub enum Error {
	Simple(ErrorKind),
	SimpleMessage(ErrorKind, ErrorMessage),

	Os(i32),
	Io(io::Error),
	Custom(ErrorKind, Box<dyn error::Error + Send + Sync>)
}

impl Error {
	pub fn new<M: Into<ErrorMessage>>(kind: ErrorKind, message: M) -> Self {
		Error::SimpleMessage(kind, message.into())
	}

	pub fn from_raw_os_error(err: i32) -> Self {
		Self::Os(err)
	}

	pub fn raw_os_error(&self) -> Option<i32> {
		match self {
			Self::Os(code) => Some(*code),
			_ => None
		}
	}

	pub fn os_error(&self) -> Option<ErrorCodes> {
		match self {
			Self::Os(code) => Some(ErrorCodes::from(*code)),
			_ => None
		}
	}

	pub fn kind(&self) -> ErrorKind {
		match self {
			Self::Simple(kind) => *kind,
			Self::SimpleMessage(kind, _) => *kind,

			Self::Os(code) => ErrorCodes::from(*code).kind(),
			Self::Io(err) => err.kind(),
			Self::Custom(kind, _) => *kind
		}
	}

	pub fn is_interrupted(&self) -> bool {
		self.kind() == ErrorKind::Interrupted
	}

	fn custom(kind: ErrorKind, err: Box<dyn error::Error + Send + Sync>) -> Self {
		match err.downcast() {
			Ok(this) => *this,
			Err(err) => Self::Custom(kind, err)
		}
	}

	pub fn other<E: Into<Box<dyn error::Error + Send + Sync>>>(value: E) -> Self {
		Self::custom(ErrorKind::Other, value.into())
	}

	pub fn invalid_input_error<E: Into<Box<dyn error::Error + Send + Sync>>>(value: E) -> Self {
		Self::custom(ErrorKind::InvalidInput, value.into())
	}

	pub fn invalid_data_error<E: Into<Box<dyn error::Error + Send + Sync>>>(value: E) -> Self {
		Self::custom(ErrorKind::InvalidData, value.into())
	}

	pub fn interrupted() -> Self {
		Self::new(ErrorKind::Interrupted, "Interrupted")
	}
}

impl From<io::Error> for Error {
	fn from(value: io::Error) -> Self {
		if let Some(code) = value.raw_os_error() {
			Self::Os(code)
		} else if value.get_ref().is_some() {
			let kind = value.kind();
			let err = value.into_inner().unwrap();

			Self::Custom(kind, err)
		} else {
			Self::Io(value)
		}
	}
}

impl From<Error> for io::Error {
	fn from(value: Error) -> Self {
		Self::new(value.kind(), value)
	}
}

impl Debug for Error {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		match self {
			Error::Simple(kind) => fmt.debug_struct("Error").field("kind", kind).finish(),
			Error::SimpleMessage(kind, message) => fmt
				.debug_struct("Error")
				.field("kind", kind)
				.field("message", &message.as_ref())
				.finish(),
			Error::Custom(kind, message) => fmt
				.debug_struct("Error")
				.field("kind", kind)
				.field("message", message)
				.finish(),
			Error::Os(code) => fmt
				.debug_struct("Os")
				.field("code", code)
				.field("kind", &ErrorCodes::from(*code).kind())
				.field("message", &ErrorCodes::from(*code).as_str())
				.finish(),
			Error::Io(io) => Debug::fmt(io, fmt)
		}
	}
}

impl Display for Error {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		match self {
			Error::Simple(kind) => write!(fmt, "{}", kind),
			Error::SimpleMessage(_, message) => Display::fmt(message.as_ref(), fmt),
			Error::Custom(_, message) => Display::fmt(message, fmt),
			Error::Os(code) => write!(
				fmt,
				"{} (os error {})",
				ErrorCodes::from(*code).as_str(),
				code
			),
			Error::Io(io) => Display::fmt(io, fmt)
		}
	}
}

impl error::Error for Error {
	#[allow(deprecated)]
	fn description(&self) -> &str {
		match self {
			Error::Os(code) => ErrorCodes::from(*code).as_str(),
			Error::SimpleMessage(_, message) => message.as_ref(),
			Error::Simple(kind) => ConstPtr::from(io::Error::from(*kind).description()).into_ref(),
			Error::Custom(_, error) => error.description(),
			Error::Io(io) => io.description()
		}
	}

	#[allow(deprecated)]
	fn cause(&self) -> Option<&dyn error::Error> {
		match self {
			Error::Os(_) | Error::Simple(_) | Error::SimpleMessage(..) => None,
			Error::Io(io) => io.cause(),
			Error::Custom(_, error) => error.cause()
		}
	}

	#[allow(deprecated)]
	fn source(&self) -> Option<&(dyn error::Error + 'static)> {
		match self {
			Error::Os(_) | Error::Simple(_) | Error::SimpleMessage(..) => None,
			Error::Io(io) => io.source(),
			Error::Custom(_, error) => error.source()
		}
	}
}
