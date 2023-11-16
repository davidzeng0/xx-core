use std::{
	error,
	fmt::{self, Debug, Display, Formatter},
	io,
	mem::transmute,
	result
};

use crate::os::error::ErrorCodes;

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
		self.raw_os_error().map(ErrorCodes::from_raw_os_error)
	}

	pub fn kind(&self) -> ErrorKind {
		match self {
			Self::Simple(kind) => *kind,
			Self::SimpleMessage(kind, _) => *kind,

			Self::Os(code) => ErrorCodes::from_raw_os_error(*code).kind(),
			Self::Io(err) => err.kind(),
			Self::Custom(kind, _) => *kind
		}
	}

	pub fn is_interrupted(&self) -> bool {
		self.kind() == ErrorKind::Interrupted
	}

	pub fn map_as(kind: ErrorKind, err: Box<dyn error::Error + Send + Sync>) -> Self {
		match err.downcast() {
			Ok(this) => *this,
			Err(err) => Self::Custom(kind, err)
		}
	}

	pub fn map_as_other<E: Into<Box<dyn error::Error + Send + Sync>>>(value: E) -> Self {
		Self::map_as(ErrorKind::Other, value.into())
	}

	pub fn map_as_invalid_input<E: Into<Box<dyn error::Error + Send + Sync>>>(value: E) -> Self {
		Self::map_as(ErrorKind::InvalidInput, value.into())
	}

	pub fn map_as_invalid_data<E: Into<Box<dyn error::Error + Send + Sync>>>(value: E) -> Self {
		Self::map_as(ErrorKind::InvalidData, value.into())
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

			Self::map_as(kind, err)
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
				.field("kind", &ErrorCodes::from_raw_os_error(*code).kind())
				.field("message", &ErrorCodes::from_raw_os_error(*code).as_str())
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
				ErrorCodes::from_raw_os_error(*code).as_str(),
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
			Error::Os(code) => ErrorCodes::from_raw_os_error(*code).as_str(),
			Error::SimpleMessage(_, message) => message.as_ref(),
			/* io error desc is a static str */
			Error::Simple(kind) => unsafe { transmute(io::Error::from(*kind).description()) },
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
