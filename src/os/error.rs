use std::io;

use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crate::error::{Error, ErrorKind, Result};

#[repr(i32)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum ErrorCodes {
	/// Unknown error
	Unknown = -1,

	/// Success
	Ok      = 0,

	/// Operation not permitted
	Perm,

	/// No such file or directory
	NoEnt,

	/// No such process
	Srch,

	/// Interrupted system call
	Intr,

	/// I/O error
	Io,

	/// No such device or address
	Nxio,

	/// Argument list too long
	TooBig,

	/// Exec format error
	NoExec,

	/// Bad file number
	BadF,

	/// No child processes
	Child,

	/// Try again
	Again,

	/// Out of memory
	NoMem,

	/// Permission denied
	Acces,

	/// Bad address
	Fault,

	/// Block device required
	NotBlk,

	/// Device or resource busy
	Busy,

	/// File exists
	Exist,

	/// Cross-device link
	XDev,

	/// No such device
	NoDev,

	/// Not a directory
	NotDir,

	/// Is a directory
	IsDir,

	/// Invalid argument
	Inval,

	/// File table overflow
	NFile,

	/// Too many open files
	MFile,

	/// Not a typewriter
	NotTy,

	/// Text file busy
	TxtBsy,

	/// File too large
	FBig,

	/// No space left on device
	NoSpc,

	/// Illegal seek
	SPipe,

	/// Read-only file system
	Rofs,

	/// Too many links
	MLink,

	/// Broken pipe
	Pipe,

	/// Math argument out of domain of func
	Dom,

	/// Math result not representable
	Range,

	/// Dead lock
	Deadlock,

	/// File name too long
	NameTooLong,

	/// No record locks available
	NoLck,

	/// Invalid system call number
	NoSys,

	/// Directory not empty
	NotEmpty,

	/// Too many symbolic links encountered
	Loop,

	/// No message of desired type
	Nomsg   = 42,

	/// Identifier removed
	Idrm,

	/// Channel number out of range
	Chrng,

	/// Level 2 not synchronized
	L2nsync,

	/// Level 3 halted
	L3hlt,

	/// Level 3 reset
	L3rst,

	/// Link number out of range
	Lnrng,

	/// Protocol driver not attached
	Unatch,

	/// No CSI structure available
	Nocsi,

	/// Level 2 halted
	L2hlt,

	/// Invalid exchange
	Bade,

	/// Invalid request descriptor
	Badr,

	/// Exchange full
	Xfull,

	/// No anode
	Noano,

	/// Invalid request code
	Badrqc,

	/// Invalid slot
	Badslt,

	/// Bad font file format
	BFont   = 59,

	/// Device not a stream
	NoStr,

	/// No data available
	NoData,

	/// Timer expired
	Time,

	/// Out of streams resources
	NoSr,

	/// Machine is not on the network
	NoNet,

	/// Package not installed
	NoPkg,

	/// Object is remote
	Remote,

	/// Link has been severed
	NoLink,

	/// Advertise error
	Adv,

	/// Srmount error
	Srmnt,

	/// Communication error on send
	Comm,

	/// Protocol error
	Proto,

	/// Multihop attempted
	Multihop,

	/// RFS specific error
	Dotdot,

	/// Not a data message
	BadMsg,

	/// Value too large for defined data type
	Overflow,

	/// Name not unique on network
	NotUniq,

	/// File descriptor in bad state
	BadFd,

	/// Remote address changed
	RemChg,

	/// Can not access a needed shared library
	LibAcc,

	/// Accessing a corrupted shared library
	LibBad,

	/// .lib section in a.out corrupted
	LibScn,

	/// Attempting to link in too many shared libraries
	LibMax,

	/// Cannot exec a shared library directly
	LibExec,

	/// Illegal byte sequence
	IlSeq,

	/// Interrupted system call should be restarted
	Restart,

	/// Streams pipe error
	StrPipe,

	/// Too many users
	Users,

	/// Socket operation on non-socket
	NotSock,

	/// Destination address required
	DestAddrReq,

	/// Message too long
	MsgSize,

	/// Protocol wrong type for socket
	Prototype,

	/// Protocol not available
	NoProtoOpt,

	/// Protocol not supported
	ProtoNoSupport,

	/// Socket type not supported
	SocktNoSupport,

	/// Operation not supported on transport endpoint
	OpNotSupp,

	/// Protocol family not supported
	PfNoSupport,

	/// Address family not supported by protocol
	AfNoSupport,

	/// Address already in use
	AddrInUse,

	/// Cannot assign requested address
	AddrNotAvail,

	/// Network is down
	NetDown,

	/// Network is unreachable
	NetUnreach,

	/// Network dropped connection because of reset
	NetReset,

	/// Software caused connection abort
	ConnAborted,

	/// Connection reset by peer
	ConnReset,

	/// No buffer space available
	NoBufs,

	/// Transport endpoint is already connected
	IsConn,

	/// Transport endpoint is not connected
	NotConn,

	/// Cannot send after transport endpoint shutdown
	Shutdown,

	/// Too many references: cannot splice
	TooManyRefs,

	/// Connection timed out
	TimedOut,

	/// Connection refused
	ConnRefused,

	/// Host is down
	HostDown,

	/// No route to host
	HostUnreach,

	/// Operation already in progress
	Already,

	/// Operation now in progress
	InProgress,

	/// Stale file handle
	Stale,

	/// Structure needs cleaning
	UClean,

	/// Not a XENIX named type file
	NotNam,

	/// No XENIX semaphores available
	NAvail,

	/// Is a named type file
	IsNam,

	/// Remote I/O error
	RemoteIo,

	/// Quota exceeded
	DQuot,

	/// No medium found
	NoMedium,

	/// Wrong medium type
	MediumType,

	/// Operation Canceled
	Canceled,

	/// Required key not available
	NoKey,

	/// Key has expired
	KeyExpired,

	/// Key has been revoked
	KeyRevoked,

	/// Key was rejected by service
	KeyRejected,

	/// Owner died
	OwnerDead,

	/// State not recoverable
	NotRecoverable,

	/// Operation not possible due to RF-kill
	RfKill,

	/// Memory page has hardware error
	HwPoison
}

#[allow(non_upper_case_globals)]
impl ErrorCodes {
	pub const DeadLk: ErrorCodes = ErrorCodes::Deadlock;
	pub const WouldBlock: ErrorCodes = ErrorCodes::Again;

	pub fn from_raw_os_error(value: i32) -> Self {
		Self::from_i32(value).unwrap_or(Self::Unknown)
	}

	pub fn kind(&self) -> ErrorKind {
		match self {
			ErrorCodes::Canceled => ErrorKind::Interrupted,
			_ => io::Error::from_raw_os_error(*self as i32).kind()
		}
	}

	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Unknown => "Unknown error",
			Self::Ok => "OK",
			Self::Perm => "Operation not permitted",
			Self::NoEnt => "No such file or directory",
			Self::Srch => "No such process",
			Self::Intr => "Interrupted system call",
			Self::Io => "Input/output error",
			Self::Nxio => "No such device or address",
			Self::TooBig => "Argument list too long",
			Self::NoExec => "Exec format error",
			Self::BadF => "Bad file descriptor",
			Self::Child => "No child processes",
			Self::Deadlock => "Resource deadlock avoided",
			Self::NoMem => "Cannot allocate memory",
			Self::Acces => "Permission denied",
			Self::Fault => "Bad address",
			Self::NotBlk => "Block device required",
			Self::Busy => "Device or resource busy",
			Self::Exist => "File exists",
			Self::XDev => "Invalid cross-device link",
			Self::NoDev => "No such device",
			Self::NotDir => "Not a directory",
			Self::IsDir => "Is a directory",
			Self::Inval => "Invalid argument",
			Self::MFile => "Too many open files",
			Self::NFile => "Too many open files in system",
			Self::NotTy => "Inappropriate ioctl for device",
			Self::TxtBsy => "Text file busy",
			Self::FBig => "File too large",
			Self::NoSpc => "No space left on device",
			Self::SPipe => "Illegal seek",
			Self::Rofs => "Read-only file system",
			Self::MLink => "Too many links",
			Self::Pipe => "Broken pipe",
			Self::Dom => "Numerical argument out of domain",
			Self::Range => "Numerical result out of range",
			Self::Again => "Resource temporarily unavailable",
			Self::InProgress => "Operation now in progress",
			Self::Already => "Operation already in progress",
			Self::NotSock => "Socket operation on non-socket",
			Self::MsgSize => "Message too long",
			Self::Prototype => "Protocol wrong type for socket",
			Self::NoProtoOpt => "Protocol not available",
			Self::ProtoNoSupport => "Protocol not supported",
			Self::SocktNoSupport => "Socket type not supported",
			Self::OpNotSupp => "Operation not supported",
			Self::PfNoSupport => "Protocol family not supported",
			Self::AfNoSupport => "Address family not supported by protocol",
			Self::AddrInUse => "Address already in use",
			Self::AddrNotAvail => "Cannot assign requested address",
			Self::NetDown => "Network is down",
			Self::NetUnreach => "Network is unreachable",
			Self::NetReset => "Network dropped connection on reset",
			Self::ConnAborted => "Software caused connection abort",
			Self::ConnReset => "Connection reset by peer",
			Self::NoBufs => "No buffer space available",
			Self::IsConn => "Transport endpoint is already connected",
			Self::NotConn => "Transport endpoint is not connected",
			Self::DestAddrReq => "Destination address required",
			Self::Shutdown => "Cannot send after transport endpoint shutdown",
			Self::TooManyRefs => "Too many references: cannot splice",
			Self::TimedOut => "Connection timed out",
			Self::ConnRefused => "Connection refused",
			Self::Loop => "Too many levels of symbolic links",
			Self::NameTooLong => "File name too long",
			Self::HostDown => "Host is down",
			Self::HostUnreach => "No route to host",
			Self::NotEmpty => "Directory not empty",
			Self::Users => "Too many users",
			Self::DQuot => "Disk quota exceeded",
			Self::Stale => "Stale file handle",
			Self::Remote => "Object is remote",
			Self::NoLck => "No locks available",
			Self::NoSys => "Not implemented",
			Self::IlSeq => "Invalid or incomplete multibyte or wide character",
			Self::BadMsg => "Bad message",
			Self::Idrm => "Identifier removed",
			Self::Multihop => "Multihop attempted",
			Self::NoData => "No data available",
			Self::NoLink => "Link has been severed",
			Self::Nomsg => "No message of desired type",
			Self::NoSr => "Out of streams resources",
			Self::NoStr => "Device not a stream",
			Self::Overflow => "Value too large for defined data type",
			Self::Proto => "Protocol error",
			Self::Time => "Timer expired",
			Self::Canceled => "Operation canceled",
			Self::OwnerDead => "Owner died",
			Self::NotRecoverable => "State not recoverable",
			Self::Restart => "Interrupted system call should be restarted",
			Self::Chrng => "Channel number out of range",
			Self::L2nsync => "Level 2 not synchronized",
			Self::L3hlt => "Level 3 halted",
			Self::L3rst => "Level 3 reset",
			Self::Lnrng => "Link number out of range",
			Self::Unatch => "Protocol driver not attached",
			Self::Nocsi => "No CSI structure available",
			Self::L2hlt => "Level 2 halted",
			Self::Bade => "Invalid exchange",
			Self::Badr => "Invalid request descriptor",
			Self::Xfull => "Exchange full",
			Self::Noano => "No anode",
			Self::Badrqc => "Invalid request code",
			Self::Badslt => "Invalid slot",
			Self::BFont => "Bad font file format",
			Self::NoNet => "Machine is not on the network",
			Self::NoPkg => "Package not installed",
			Self::Adv => "Advertise error",
			Self::Srmnt => "Srmount error",
			Self::Comm => "Communication error on send",
			Self::Dotdot => "RFS specific error",
			Self::NotUniq => "Name not unique on network",
			Self::BadFd => "File descriptor in bad state",
			Self::RemChg => "Remote address changed",
			Self::LibAcc => "Can not access a needed shared library",
			Self::LibBad => "Accessing a corrupted shared library",
			Self::LibScn => ".lib section in a.out corrupted",
			Self::LibMax => "Attempting to link in too many shared libraries",
			Self::LibExec => "Cannot exec a shared library directly",
			Self::StrPipe => "Streams pipe error",
			Self::UClean => "Structure needs cleaning",
			Self::NotNam => "Not a XENIX named type file",
			Self::NAvail => "No XENIX semaphores available",
			Self::IsNam => "Is a named type file",
			Self::RemoteIo => "Remote I/O error",
			Self::NoMedium => "No medium found",
			Self::MediumType => "Wrong medium type",
			Self::NoKey => "Required key not available",
			Self::KeyExpired => "Key has expired",
			Self::KeyRevoked => "Key has been revoked",
			Self::KeyRejected => "Key was rejected by service",
			Self::RfKill => "Operation not possible due to RF-kill",
			Self::HwPoison => "Memory page has hardware error"
		}
	}
}

pub fn result_from_libc(result: isize) -> Result<isize> {
	if result >= 0 {
		return Ok(result);
	}

	Err(io::Error::last_os_error().into())
}

pub fn result_from_int(result: isize) -> Result<isize> {
	if result >= 0 {
		return Ok(result);
	}

	Err(Error::from_raw_os_error(-result as i32))
}

pub fn result_from_ptr(result: isize) -> Result<usize> {
	let err = -4096isize as usize;

	if result as usize <= err {
		return Ok(result as usize);
	}

	Err(Error::from_raw_os_error(result as i32))
}
