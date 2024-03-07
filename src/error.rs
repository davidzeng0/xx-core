use std::{
	error,
	fmt::{self, Debug, Display, Formatter},
	io,
	mem::transmute,
	result
};

use crate::os::error::OsError;

pub type Result<T> = result::Result<T, Error>;
pub use io::ErrorKind;

pub use crate::macros::compact_error;

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

pub struct Simple {
	kind: ErrorKind
}

pub struct Compact {
	strings: &'static [&'static str],
	kind: ErrorKind,
	ordinal: u32
}

impl Compact {
	fn name(&self) -> &'static str {
		self.strings[0]
	}

	fn variant(&self) -> &'static str {
		self.strings[self.ordinal as usize * 2 + 1]
	}

	fn message(&self) -> &'static str {
		self.strings[self.ordinal as usize * 2 + 2]
	}
}

pub struct Extern {
	kind: ErrorKind,
	data: Box<dyn error::Error + Send + Sync>
}

pub trait CompactError: Copy {
	const STRINGS: &'static [&'static str];

	fn as_err(&self) -> Error {
		(*self).into()
	}

	fn as_err_with_msg<M>(&self, message: M) -> Error
	where
		M: Into<ErrorMessage>
	{
		Error::compact(*self, Some(message))
	}

	fn kind(&self) -> ErrorKind;
	fn ordinal(&self) -> u32;

	unsafe fn from_ordinal_unchecked(ordinal: u32) -> Self;
}

enum Repr {
	Os(OsError),
	Simple(Simple),
	Compact(Compact),
	Extern(Extern)
}

pub struct Error {
	repr: Repr,
	message: Option<ErrorMessage>
}

impl Error {
	pub fn simple<M>(kind: ErrorKind, message: Option<M>) -> Self
	where
		M: Into<ErrorMessage>
	{
		Self {
			repr: Repr::Simple(Simple { kind }),
			message: message.map(Into::into)
		}
	}

	pub fn from_raw_os_error(err: i32) -> Self {
		Self {
			repr: Repr::Os(OsError::from_raw(err)),
			message: None
		}
	}

	pub fn os_error(&self) -> Option<OsError> {
		match &self.repr {
			Repr::Os(code) => Some(*code),
			_ => None
		}
	}

	pub fn kind(&self) -> ErrorKind {
		match &self.repr {
			Repr::Os(code) => code.kind(),
			Repr::Simple(simple) => simple.kind,
			Repr::Compact(compact) => compact.kind,
			Repr::Extern(external) => external.kind
		}
	}

	pub fn is_interrupted(&self) -> bool {
		self.kind() == ErrorKind::Interrupted
	}

	pub fn compact<T, M>(value: T, message: Option<M>) -> Self
	where
		T: CompactError,
		M: Into<ErrorMessage>
	{
		Self {
			repr: Repr::Compact(Compact {
				strings: T::STRINGS,
				kind: value.kind(),
				ordinal: value.ordinal()
			}),
			message: message.map(Into::into)
		}
	}

	pub fn map_as(kind: ErrorKind, err: Box<dyn error::Error + Send + Sync>) -> Self {
		match err.downcast() {
			Ok(this) => *this,
			Err(err) => Self {
				repr: Repr::Extern(Extern { kind, data: err }),
				message: None
			}
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

	pub fn downcast<T: CompactError>(&self) -> Option<T> {
		match &self.repr {
			Repr::Compact(compact) if compact.strings.as_ptr() == T::STRINGS.as_ptr() => {
				Some(unsafe { T::from_ordinal_unchecked(compact.ordinal) })
			}
			_ => None
		}
	}
}

impl<T: CompactError> PartialEq<T> for Error {
	fn eq(&self, error: &T) -> bool {
		let cur = self.downcast::<T>();

		match cur {
			Some(err) if err.ordinal() == error.ordinal() => true,
			_ => true
		}
	}
}

impl From<Error> for io::Error {
	fn from(value: Error) -> Self {
		Self::new(value.kind(), value)
	}
}

impl From<io::Error> for Error {
	fn from(value: io::Error) -> Self {
		if let Some(code) = value.raw_os_error() {
			Self::from_raw_os_error(code)
		} else if value.get_ref().is_some() {
			let kind = value.kind();
			let err = value.into_inner().unwrap();

			Self::map_as(kind, err)
		} else {
			#[allow(deprecated)]
			let description: &'static str = unsafe { transmute(error::Error::description(&value)) };

			Self::simple(value.kind(), Some(description))
		}
	}
}

impl From<OsError> for Error {
	fn from(value: OsError) -> Self {
		Self { repr: Repr::Os(value), message: None }
	}
}

impl<T: CompactError> From<T> for Error {
	fn from(value: T) -> Self {
		Self::compact(value, None::<&'static str>)
	}
}

impl Debug for Error {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		match &self.repr {
			Repr::Simple(simple) => {
				let mut debug = fmt.debug_struct("Error");

				debug.field("kind", &simple.kind);

				if let Some(message) = &self.message {
					debug.field("message", &message.as_ref());
				}

				debug.finish()
			}
			Repr::Compact(compact) => fmt
				.debug_struct(compact.name())
				.field("what", &compact.variant())
				.field(
					"message",
					&self
						.message
						.as_ref()
						.map(AsRef::as_ref)
						.unwrap_or(compact.message())
				)
				.finish(),
			Repr::Extern(external) => fmt
				.debug_struct("Extern")
				.field("kind", &external.kind)
				.field("data", &external.data)
				.finish(),
			Repr::Os(code) => fmt
				.debug_struct("Os")
				.field("code", &(*code as i32))
				.field("kind", &code.kind())
				.field("message", &code.as_str())
				.finish()
		}
	}
}

impl Display for Error {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		if let Some(message) = &self.message {
			return Display::fmt(message.as_ref(), fmt);
		}

		match &self.repr {
			Repr::Simple(simple) => write!(fmt, "{}", simple.kind),
			Repr::Compact(compact) => Display::fmt(&compact.message(), fmt),
			Repr::Extern(external) => Display::fmt(&external.data, fmt),
			Repr::Os(code) => write!(fmt, "{} (os error {})", code.as_str(), *code as i32)
		}
	}
}

impl error::Error for Error {
	#[allow(deprecated)]
	fn description(&self) -> &str {
		match &self.repr {
			Repr::Os(code) => code.as_str(),
			Repr::Simple(simple) => {
				if let Some(message) = &self.message {
					message.as_ref()
				} else {
					unsafe { transmute(io::Error::from(simple.kind).description()) }
				}
			}
			Repr::Compact(compact) => compact.message(),
			Repr::Extern(external) => external.data.description()
		}
	}

	#[allow(deprecated)]
	fn cause(&self) -> Option<&dyn error::Error> {
		match &self.repr {
			Repr::Os(_) | Repr::Simple(..) | Repr::Compact(_) => None,
			Repr::Extern(external) => external.data.cause()
		}
	}

	fn source(&self) -> Option<&(dyn error::Error + 'static)> {
		match &self.repr {
			Repr::Os(_) | Repr::Simple(..) | Repr::Compact(_) => None,
			Repr::Extern(external) => external.data.source()
		}
	}
}

#[compact_error]
pub enum Core {
	Interrupted    = (ErrorKind::Interrupted, "Interrupted"),
	WriteZero      = (ErrorKind::WriteZero, "Write EOF"),
	InvalidUtf8    = (ErrorKind::InvalidData, "Invalid UTF-8 found in stream"),
	UnexpectedEof  = (ErrorKind::UnexpectedEof, "Unexpected EOF"),
	Overflow       = (ErrorKind::InvalidInput, "Integer overflow"),
	OutOfMemory    = (ErrorKind::OutOfMemory, "Out of memory"),
	NoAddresses    = (ErrorKind::InvalidInput, "Address list empty"),
	InvalidCStr    = (ErrorKind::InvalidInput, "Path string contained a null byte"),
	ConnectTimeout = (ErrorKind::TimedOut, "Connection timed out"),
	Shutdown       = (ErrorKind::NotConnected, "Endpoint is shutdown"),
	FormatterError = (ErrorKind::Other, "Formatter error"),
	Pending        = (ErrorKind::Other, "Operation in progress")
}
