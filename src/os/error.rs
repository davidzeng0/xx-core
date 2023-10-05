use std::io::{Error, Result};

pub fn result_from_libc(result: isize) -> Result<isize> {
	if result >= 0 {
		return Ok(result);
	}

	Err(Error::last_os_error())
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

#[repr(i32)]
pub enum ErrorCodes {
	/// Operation not permitted
	Perm = 1,

	/// No such file or directory
	NoEnt = 2,

	/// No such process
	Srch = 3,

	/// Interrupted system call
	Intr = 4,

	/// I/O error
	Io = 5,

	/// No such device or address
	Nxio = 6,

	/// Argument list too long
	TooBig = 7,

	/// Exec format error
	NoExec = 8,

	/// Bad file number
	BadF = 9,

	/// No child processes
	Child = 10,

	/// Try again
	Again = 11,

	/// Out of memory
	NoMem = 12,

	/// Permission denied
	Acces = 13,

	/// Bad address
	Fault = 14,

	/// Block device required
	NotBlk = 15,

	/// Device or resource busy
	Busy = 16,

	/// File exists
	Exist = 17,

	/// Cross-device link
	XDev = 18,

	/// No such device
	NoDev = 19,

	/// Not a directory
	NotDir = 20,

	/// Is a directory
	IsDir = 21,

	/// Invalid argument
	Inval = 22,

	/// File table overflow
	NFile = 23,

	/// Too many open files
	MFile = 24,

	/// Not a typewriter
	NotTy = 25,

	/// Text file busy
	TxtBsy = 26,

	/// File too large
	FBig = 27,

	/// No space left on device
	NoSpc = 28,

	/// Illegal seek
	SPipe = 29,

	/// Read-only file system
	Rofs = 30,

	/// Too many links
	MLink = 31,

	/// Broken pipe
	Pipe = 32,

	/// Math argument out of domain of func
	Dom = 33,

	/// Math result not representable
	Range = 34,

	/// File name too long
	NameTooLong = 36,

	/// No record locks available
	NoLck = 37,

	/// Invalid system call number
	NoSys = 38,

	/// Directory not empty
	NotEmpty = 39,

	/// Too many symbolic links encountered
	Loop = 40,

	/// No message of desired type
	Nomsg = 42,

	/// Identifier removed
	Idrm = 43,

	/// Channel number out of range
	Chrng = 44,

	/// Level 2 not synchronized
	L2nsync = 45,

	/// Level 3 halted
	L3hlt = 46,

	/// Level 3 reset
	L3rst = 47,

	/// Link number out of range
	Lnrng = 48,

	/// Protocol driver not attached
	Unatch = 49,

	/// No CSI structure available
	Nocsi = 50,

	/// Level 2 halted
	L2hlt = 51,

	/// Invalid exchange
	Bade = 52,

	/// Invalid request descriptor
	Badr = 53,

	/// Exchange full
	Xfull = 54,

	/// No anode
	Noano = 55,

	/// Invalid request code
	Badrqc = 56,

	/// Invalid slot
	Badslt = 57,

	/// Dead lock
	Deadlock = 35,

	/// Bad font file format
	BFont = 59,

	/// Device not a stream
	NoStr = 60,

	/// No data available
	NoData = 61,

	/// Timer expired
	Time = 62,

	/// Out of streams resources
	NoSr = 63,

	/// Machine is not on the network
	NoNet = 64,

	/// Package not installed
	NoPkg = 65,

	/// Object is remote
	Remote = 66,

	/// Link has been severed
	NoLink = 67,

	/// Advertise error
	Adv = 68,

	/// Srmount error
	Srmnt = 69,

	/// Communication error on send
	Comm = 70,

	/// Protocol error
	Proto = 71,

	/// Multihop attempted
	Multihop = 72,

	/// RFS specific error
	Dotdot = 73,

	/// Not a data message
	BadMsg = 74,

	/// Value too large for defined data type
	Overflow = 75,

	/// Name not unique on network
	NotUniq = 76,

	/// File descriptor in bad state
	BadFd = 77,

	/// Remote address changed
	RemChg = 78,

	/// Can not access a needed shared library
	LibAcc = 79,

	/// Accessing a corrupted shared library
	LibBad = 80,

	/// .lib section in a.out corrupted
	LibScn = 81,

	/// Attempting to link in too many shared libraries
	LibMax = 82,

	/// Cannot exec a shared library directly
	LibExec = 83,

	/// Illegal byte sequence
	IlSeq = 84,

	/// Interrupted system call should be restarted
	Restart = 85,

	/// Streams pipe error
	StrPipe = 86,

	/// Too many users
	Users = 87,

	/// Socket operation on non-socket
	NotSock = 88,

	/// Destination address required
	DestAddrReq = 89,

	/// Message too long
	MsgSize = 90,

	/// Protocol wrong type for socket
	Prototype = 91,

	/// Protocol not available
	NoProtoOpt = 92,

	/// Protocol not supported
	ProtoNoSupport = 93,

	/// Socket type not supported
	SocktNoSupport = 94,

	/// Operation not supported on transport endpoint
	OpNotSupp = 95,

	/// Protocol family not supported
	PfNoSupport = 96,

	/// Address family not supported by protocol
	AfNoSupport = 97,

	/// Address already in use
	AddrInUse = 98,

	/// Cannot assign requested address
	AddrNotAvail = 99,

	/// Network is down
	NetDown = 100,

	/// Network is unreachable
	NetUnreach = 101,

	/// Network dropped connection because of reset
	NetReset = 102,

	/// Software caused connection abort
	ConnAborted = 103,

	/// Connection reset by peer
	ConnReset = 104,

	/// No buffer space available
	NoBufs = 105,

	/// Transport endpoint is already connected
	IsConn = 106,

	/// Transport endpoint is not connected
	NotConn = 107,

	/// Cannot send after transport endpoint shutdown
	Shutdown = 108,

	/// Too many references: cannot splice
	TooManyRefs = 109,

	/// Connection timed out
	TimedOut = 110,

	/// Connection refused
	ConnRefused = 111,

	/// Host is down
	HostDown = 112,

	/// No route to host
	HostUnreach = 113,

	/// Operation already in progress
	Already = 114,

	/// Operation now in progress
	InProgress = 115,

	/// Stale file handle
	Stale = 116,

	/// Structure needs cleaning
	UClean = 117,

	/// Not a XENIX named type file
	NotNam = 118,

	/// No XENIX semaphores available
	NAvail = 119,

	/// Is a named type file
	IsNam = 120,

	/// Remote I/O error
	RemoteIo = 121,

	/// Quota exceeded
	DQuot = 122,

	/// No medium found
	NoMedium = 123,

	/// Wrong medium type
	MediumType = 124,

	/// Operation Canceled
	Canceled = 125,

	/// Required key not available
	NoKey = 126,

	/// Key has expired
	KeyExpired = 127,

	/// Key has been revoked
	KeyRevoked = 128,

	/// Key was rejected by service
	KeyRejected = 129,

	/// Owner died
	OwnerDead = 130,

	/// State not recoverable
	NotRecoverable = 131,

	/// Operation not possible due to RF-kill
	RfKill = 132,

	/// Memory page has hardware error
	HwPoison = 133
}

#[allow(non_upper_case_globals)]
impl ErrorCodes {
	pub const WouldBlock: ErrorCodes = ErrorCodes::Again;
	pub const DeadLk: ErrorCodes = ErrorCodes::Deadlock;
}
