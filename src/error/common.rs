use super::*;

#[non_exhaustive]
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum ErrorKind {
	NotFound,
	PermissionDenied,
	ConnectionRefused,
	ConnectionReset,
	HostUnreachable,
	NetworkUnreachable,
	ConnectionAborted,
	NotConnected,
	AddrInUse,
	AddrNotAvailable,
	NetworkDown,
	BrokenPipe,
	AlreadyExists,
	WouldBlock,
	NotADirectory,
	IsADirectory,
	DirectoryNotEmpty,
	ReadOnlyFilesystem,
	FileSystemLoop,
	StaleNetworkFileHandle,
	InvalidInput,
	InvalidData,
	TimedOut,
	WriteZero,
	StorageFull,
	NotSeekable,
	DiskQuotaExceeded,
	FileTooLarge,
	ResourceBusy,
	ExecutableFileBusy,
	Deadlock,
	CrossesDevices,
	TooManyLinks,
	InvalidFilename,
	ArgumentListTooLong,
	Interrupted,
	Unsupported,
	UnexpectedEof,
	OutOfMemory,
	Overflow,
	Shutdown,
	FormatterError,
	Unimplemented,
	AlreadyInProgress,
	NoData,
	Other,
	Uncategorized
}

impl ErrorKind {
	#[must_use]
	pub const fn as_str(&self) -> &'static str {
		use ErrorKind::*;

		match *self {
			NotFound => "Entity not found",
			PermissionDenied => "Permission denied",
			ConnectionRefused => "Connection refused",
			ConnectionReset => "Connection reset",
			HostUnreachable => "Host unreachable",
			NetworkUnreachable => "Network unreachable",
			ConnectionAborted => "Connection aborted",
			NotConnected => "Not connected",
			AddrInUse => "Address in use",
			AddrNotAvailable => "Address not available",
			NetworkDown => "Network down",
			BrokenPipe => "Broken pipe",
			AlreadyExists => "Entity already exists",
			WouldBlock => "Operation would block",
			NotADirectory => "Not a directory",
			IsADirectory => "Is a directory",
			DirectoryNotEmpty => "Directory not empty",
			ReadOnlyFilesystem => "Read-only file system",
			FileSystemLoop => "Too many levels of symbolic links",
			StaleNetworkFileHandle => "Stale network file handle",
			InvalidInput => "Invalid argument",
			InvalidData => "Invalid data",
			TimedOut => "Operation timed out",
			WriteZero => "Write EOF",
			StorageFull => "Storage full",
			NotSeekable => "Not seekable",
			DiskQuotaExceeded => "Disk quota exceeded",
			FileTooLarge => "File too large",
			ResourceBusy => "Resource busy",
			ExecutableFileBusy => "Executable file busy",
			Deadlock => "Dead lock",
			CrossesDevices => "Invalid cross-device link",
			TooManyLinks => "Too many links",
			InvalidFilename => "Invalid file name",
			ArgumentListTooLong => "Argument list too long",
			Interrupted => "Operation interrupted",
			Unsupported => "Unsupported",
			UnexpectedEof => "Unexpected end of file",
			OutOfMemory => "Out of memory",
			Overflow => "Overflow occurred",
			Shutdown => "Resource is shutdown",
			FormatterError => "Formatter error",
			Unimplemented => "Not implemented",
			AlreadyInProgress => "Operation already in progress",
			NoData => "No data",
			Other => "Other",
			Uncategorized => "Uncategorized"
		}
	}

	#[must_use]
	pub const fn invalid_utf8() -> &'static SimpleMessage {
		&SimpleMessage {
			kind: Self::InvalidData,
			message: "Processed invalid UTF-8"
		}
	}

	#[must_use]
	pub const fn no_addrs() -> &'static SimpleMessage {
		&SimpleMessage { kind: Self::NoData, message: "Address list empty" }
	}

	#[must_use]
	pub const fn invalid_cstr() -> &'static SimpleMessage {
		&SimpleMessage {
			kind: Self::InvalidInput,
			message: "Path string contained a null byte"
		}
	}

	#[must_use]
	pub const fn connect_timed_out() -> &'static SimpleMessage {
		&SimpleMessage { kind: Self::TimedOut, message: "Connect timed out" }
	}
}

impl Display for ErrorKind {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		fmt.write_str(self.as_str())
	}
}

impl From<OsError> for ErrorKind {
	fn from(value: OsError) -> Self {
		use ErrorKind::*;

		match value {
			OsError::TooBig => ArgumentListTooLong,
			OsError::AddrInUse => AddrInUse,
			OsError::AddrNotAvail => AddrNotAvailable,
			OsError::Busy => ResourceBusy,
			OsError::ConnAborted => ConnectionAborted,
			OsError::ConnRefused => ConnectionRefused,
			OsError::ConnReset => ConnectionReset,
			OsError::Deadlock => Deadlock,
			OsError::DQuot => DiskQuotaExceeded,
			OsError::Exist => AlreadyExists,
			OsError::FBig => FileTooLarge,
			OsError::HostUnreach => HostUnreachable,
			OsError::Intr => Interrupted,
			OsError::Inval => InvalidInput,
			OsError::IsDir => IsADirectory,
			OsError::Loop => FileSystemLoop,
			OsError::NoEnt => NotFound,
			OsError::NoMem => OutOfMemory,
			OsError::NoSpc => StorageFull,
			OsError::NoSys => Unsupported,
			OsError::MLink => TooManyLinks,
			OsError::NameTooLong => InvalidFilename,
			OsError::NetDown => NetworkDown,
			OsError::NetUnreach => NetworkUnreachable,
			OsError::NotConn => NotConnected,
			OsError::NotDir => NotADirectory,
			OsError::NotEmpty => DirectoryNotEmpty,
			OsError::Pipe => BrokenPipe,
			OsError::Rofs => ReadOnlyFilesystem,
			OsError::SPipe => NotSeekable,
			OsError::Stale => StaleNetworkFileHandle,
			OsError::TimedOut => TimedOut,
			OsError::TxtBsy => ExecutableFileBusy,
			OsError::XDev => CrossesDevices,
			OsError::Acces | OsError::Perm => PermissionDenied,
			OsError::Canceled => Interrupted,

			code if code == OsError::Again || code == OsError::WouldBlock => WouldBlock,

			_ => Uncategorized
		}
	}
}
