use std::{
	mem::{size_of, zeroed},
	os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd}
};

use enumflags2::bitflags;

use super::{
	inet::Address,
	iovec::IoVec,
	syscall::{syscall_int, SyscallNumber::*},
	tcp::TcpOption
};
use crate::{
	error::Result,
	pointer::{ConstPtr, MutPtr}
};

#[repr(u32)]
pub enum ProtocolFamily {
	/// Unspecified.
	Unspec,

	/// Local to host (pipes and file-domain).
	Local,

	/// IP protocol family.
	INet,

	/// Amateur Radio AX.25.
	Ax25,

	/// Novell Internet Protocol.
	IPx,

	/// Appletalk DDP.
	AppleTalk,

	/// Amateur radio NetROM.
	NetRom,

	/// Multiprotocol bridge.
	Bridge,

	/// ATM PVCs.
	ATMPvc,

	/// Reserved for X.25 project.
	X25,

	/// IP version 6.
	INet6,

	/// Amateur Radio X.25 PLP.
	Rose,

	/// Reserved for DECnet project.
	DecNet,

	/// Reserved for 802.2LLC project.
	NetBeui,

	/// Security callback pseudo AF.
	Security,

	/// PF_KEY key management API.
	Key,

	NetLink,

	/// Packet family.
	Packet,

	/// Ash.
	Ash,

	/// Acorn Econet.
	Econet,

	/// ATM SVCs.
	ATMSvc,

	/// RDS sockets.
	Rds,

	/// Linux SNA Project
	Sna,

	/// IRDA sockets.
	Irda,

	/// PPPoX sockets.
	PPPoX,

	/// Wanpipe API sockets.
	Wanpipe,

	/// Linux LLC.
	Llc,

	/// Native InfiniBand address.
	Ib,

	/// MPLS.
	Mpls,

	/// Controller Area Network.
	Can,

	/// TIPC sockets.
	Tipc,

	/// Bluetooth sockets.
	Bluetooth,

	/// IUCV sockets.
	Iucv,

	/// RxRPC sockets.
	RxRpc,

	/// mISDN sockets.
	Isdn,

	/// Phonet sockets.
	Phonet,

	/// IEEE 802.15.4 sockets.
	Ieee802154,

	/// CAIF sockets.
	Caif,

	/// Algorithm sockets.
	Alg,

	/// NFC sockets.
	Nfc,

	/// vSockets.
	Vsock,

	/// Kernel Connection Multiplexor.
	Kcm,

	/// Qualcomm IPC Router.
	Qipcrtr,

	/// SMC sockets.
	Smc,

	/// XDP sockets.
	Xdp,

	/// Management component transport protocol.
	Mctp
}

#[allow(non_upper_case_globals)]
impl ProtocolFamily {
	/// Another non-standard name for PF_LOCAL.
	pub const File: ProtocolFamily = ProtocolFamily::Local;
	/// Alias to emulate 4.4BSD.
	pub const Route: ProtocolFamily = ProtocolFamily::NetLink;
	/// POSIX name for PF_LOCAL.
	pub const Unix: ProtocolFamily = ProtocolFamily::Local;
}

pub type AddressFamily = ProtocolFamily;

#[repr(u32)]
pub enum SocketType {
	Stream = 1,
	Datagram,
	Raw,
	Rdm,
	SeqPacket,
	Dccp,
	Packet = 10
}

#[allow(non_upper_case_globals)]
impl SocketType {
	pub const CloExec: u32 = 80000;
	pub const NonBlock: u32 = 800;
}

#[repr(u32)]
pub enum SocketLevel {
	Socket = 1,
	Tcp    = 6,
	Raw    = 255,
	DecNet = 261,
	X25,
	Packet,
	Atm,
	Aal,
	Irda,
	NetBeui,
	Llc,
	Dccp,
	NetLink,
	Tipc,
	RxRpc,
	Pppol2tp,
	Bluetooth,
	PnPipe,
	Rds,
	Iucv,
	Caif,
	Alg,
	Nfc,
	Kcm,
	Tls,
	Xdp
}

#[repr(u32)]
pub enum SocketOption {
	Debug = 1,
	ReuseAddr,
	Type,
	Error,
	DontRoute,
	Broadcast,
	SendBufSize,
	RecvBufSize,
	KeepAlive,
	OutOfBandInline,
	NoCheck,
	Priority,
	Linger,
	BSDCompat,
	ReusePort,
	PassCred,
	PeerCred,
	RecvLowWatermark,
	SendLowWatermark,
	RecvTimeoutOld,
	SendTimeoutOld,
	SecurityAuthentication,
	SecurityEncryptionTransport,
	SecurityEncryptionNetwork,
	BindToDevice,
	AttachFilter,
	DetachFilter,
	PeerName,
	TimestampOld,
	AcceptConn,
	PeerSec,
	SendBufSizeForce,
	RecvBufSizeForce,
	PassSec,
	TimestampNanosecondsOld,
	Mark,
	TimestampingOld,
	Protocol,
	Domain,
	RxqOverflow,
	WifiStatus,
	PeekOff,
	NoFcs,
	LockFilter,
	SelectErrorQueue,
	BusyPoll,
	MaxPacingRate,
	BPFExtensions,
	IncomingCpu,
	AttachBPF,
	AttachReusePortCBPF,
	AttachReusePortEBPF,
	CnxAdvice,
	TimestampingOptStats,
	MemInfo,
	IncomingNapiId,
	Cookie,
	TimestampingPacketInfo,
	PeerGroups,
	ZeroCopy,
	TxtTime,
	BindToIfIndex,
	TimestampNew,
	TimestampNanosecondsNew,
	TimestampingNew,
	RecvTimeoutNew,
	SendTimeoutNew,
	DetachReusePortBPF,
	PreferBusyPoll,
	BusyPollBudget,
	NetNsCookie,
	BufLock,
	ReserveMem,
	TxRehash,
	RecvMark
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Shutdown {
	Read,
	Write,
	Both
}

#[bitflags]
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MessageFlag {
	OutOfBand            = 1 << 0,
	Peek                 = 1 << 1,
	DontRoute            = 1 << 2,
	ControlDataTruncated = 1 << 3,
	Proxy                = 1 << 4,
	Truncate             = 1 << 5,
	DontWait             = 1 << 6,
	EndOfRecord          = 1 << 7,
	WaitAll              = 1 << 8,
	Fin                  = 1 << 9,
	Syn                  = 1 << 10,
	Confirm              = 1 << 11,
	Reset                = 1 << 12,
	ErrorQueue           = 1 << 13,
	NoSignal             = 1 << 14,
	More                 = 1 << 15,
	WaitForOne           = 1 << 16,
	Batch                = 1 << 18,
	ZeroCopy             = 1 << 26,
	FastOpen             = 1 << 29,
	CMsgCloExec          = 1 << 30
}

#[repr(C)]
pub struct MessageHeader {
	pub address: *const (),
	pub address_len: u32,

	pub iov: *mut IoVec,
	pub iov_len: usize,

	pub control: *const (),
	pub control_len: usize,

	pub flags: u32
}

impl MessageHeader {
	pub fn new() -> Self {
		unsafe { zeroed() }
	}
}

impl Default for MessageHeader {
	fn default() -> Self {
		Self::new()
	}
}

pub fn socket(domain: u32, socket_type: u32, protocol: u32) -> Result<OwnedFd> {
	let fd = syscall_int!(Socket, domain, socket_type, protocol)?;

	Ok(unsafe { OwnedFd::from_raw_fd(fd as i32) })
}

pub fn bind_raw(socket: BorrowedFd<'_>, addr: ConstPtr<()>, addrlen: u32) -> Result<()> {
	syscall_int!(Bind, socket.as_raw_fd(), addr.as_raw_int(), addrlen)?;

	Ok(())
}

pub fn bind<A>(socket: BorrowedFd<'_>, addr: &A) -> Result<()> {
	bind_raw(socket, ConstPtr::from(addr).cast(), size_of::<A>() as u32)
}

pub fn bind_addr(socket: BorrowedFd<'_>, addr: &Address) -> Result<()> {
	match &addr {
		Address::V4(addr) => bind(socket, addr),
		Address::V6(addr) => bind(socket, addr)
	}
}

pub fn connect_raw(socket: BorrowedFd<'_>, addr: ConstPtr<()>, addrlen: u32) -> Result<()> {
	syscall_int!(Connect, socket.as_raw_fd(), addr.as_raw_int(), addrlen)?;

	Ok(())
}

pub fn connect<A>(socket: BorrowedFd<'_>, addr: &A) -> Result<()> {
	connect_raw(socket, ConstPtr::from(addr).cast(), size_of::<A>() as u32)
}

pub fn connect_addr(socket: BorrowedFd<'_>, addr: &Address) -> Result<()> {
	match &addr {
		Address::V4(addr) => connect(socket, addr),
		Address::V6(addr) => connect(socket, addr)
	}
}

pub fn accept_raw(socket: BorrowedFd<'_>, addr: MutPtr<()>, addrlen: &mut u32) -> Result<OwnedFd> {
	let fd = syscall_int!(
		Accept,
		socket.as_raw_fd(),
		addr.as_raw_int(),
		MutPtr::from(addrlen).as_raw_int()
	)?;

	Ok(unsafe { OwnedFd::from_raw_fd(fd as i32) })
}

pub fn accept<A>(socket: BorrowedFd<'_>, addr: &mut A) -> Result<(OwnedFd, u32)> {
	let mut addrlen = size_of::<A>() as u32;
	let fd = accept_raw(socket, MutPtr::from(addr).cast(), &mut addrlen)?;

	Ok((fd, addrlen))
}

pub fn sendto_raw(
	socket: BorrowedFd<'_>, buf: ConstPtr<()>, len: usize, flags: u32, dest_addr: ConstPtr<()>,
	addrlen: u32
) -> Result<usize> {
	let sent = syscall_int!(
		Sendto,
		socket.as_raw_fd(),
		buf.as_raw_int(),
		len,
		flags,
		dest_addr.as_raw_int(),
		addrlen
	)?;

	Ok(sent as usize)
}

pub fn sendto<A>(
	socket: BorrowedFd<'_>, buf: ConstPtr<()>, len: usize, flags: u32, destination: &A
) -> Result<usize> {
	sendto_raw(
		socket,
		buf,
		len,
		flags,
		ConstPtr::from(&destination).cast(),
		size_of::<A>() as u32
	)
}

pub fn send(socket: BorrowedFd<'_>, buf: ConstPtr<()>, len: usize, flags: u32) -> Result<usize> {
	sendto_raw(socket, buf, len, flags, ConstPtr::null(), 0)
}

pub fn recvfrom_raw(
	socket: BorrowedFd<'_>, buf: ConstPtr<()>, len: usize, flags: u32, addr: MutPtr<()>,
	addrlen: &mut u32
) -> Result<usize> {
	let received = syscall_int!(
		Recvfrom,
		socket.as_raw_fd(),
		buf.as_raw_int(),
		len,
		flags,
		addr.as_raw_int(),
		MutPtr::from(addrlen).as_raw_int()
	)?;

	Ok(received as usize)
}

pub fn recvfrom<A>(
	socket: BorrowedFd<'_>, buf: ConstPtr<()>, len: usize, flags: u32, addr: &mut A
) -> Result<(usize, u32)> {
	let mut addrlen = size_of::<A>() as u32;
	let recvd = recvfrom_raw(
		socket,
		buf,
		len,
		flags,
		MutPtr::from(addr).cast(),
		&mut addrlen
	)?;

	Ok((recvd, addrlen))
}

pub fn recv(socket: BorrowedFd<'_>, buf: ConstPtr<()>, len: usize, flags: u32) -> Result<usize> {
	let received = syscall_int!(
		Recvfrom,
		socket.as_raw_fd(),
		buf.as_raw_int(),
		len,
		flags,
		0,
		0
	)?;

	Ok(received as usize)
}

pub const MAX_BACKLOG: i32 = 4096;

pub fn listen(socket: BorrowedFd<'_>, backlog: i32) -> Result<()> {
	syscall_int!(Listen, socket.as_raw_fd(), backlog)?;

	Ok(())
}

pub fn shutdown(socket: BorrowedFd<'_>, how: Shutdown) -> Result<()> {
	syscall_int!(Shutdown, socket.as_raw_fd(), how)?;

	Ok(())
}

pub fn getsockopt<Opt>(
	socket: BorrowedFd<'_>, level: i32, option: i32, opt_val: &mut Opt
) -> Result<u32> {
	let res = syscall_int!(
		Setsockopt,
		socket.as_raw_fd(),
		level,
		option,
		MutPtr::from(opt_val).as_raw_int(),
		size_of::<Opt>
	)?;

	Ok(res as u32)
}

pub fn setsockopt<Opt>(
	socket: BorrowedFd<'_>, level: i32, option: i32, opt_val: &Opt
) -> Result<u32> {
	let res = syscall_int!(
		Setsockopt,
		socket.as_raw_fd(),
		level,
		option,
		ConstPtr::from(opt_val).as_raw_int(),
		size_of::<Opt>()
	)?;

	Ok(res as u32)
}

pub fn set_reuse_addr(socket: BorrowedFd<'_>, enable: bool) -> Result<()> {
	let enable = enable as i32;

	setsockopt(
		socket,
		SocketLevel::Socket as i32,
		SocketOption::ReuseAddr as i32,
		&enable
	)?;

	Ok(())
}

pub fn set_recvbuf_size(socket: BorrowedFd<'_>, size: i32) -> Result<()> {
	setsockopt(
		socket,
		SocketLevel::Socket as i32,
		SocketOption::RecvBufSize as i32,
		&size
	)?;

	Ok(())
}

pub fn set_sendbuf_size(socket: BorrowedFd<'_>, size: i32) -> Result<()> {
	setsockopt(
		socket,
		SocketLevel::Socket as i32,
		SocketOption::SendBufSize as i32,
		&size
	)?;

	Ok(())
}

pub fn set_tcp_nodelay(socket: BorrowedFd<'_>, enable: bool) -> Result<()> {
	let enable = enable as i32;

	setsockopt(
		socket,
		SocketLevel::Tcp as i32,
		TcpOption::NoDelay as i32,
		&enable
	)?;

	Ok(())
}

pub fn set_tcp_keepalive(socket: BorrowedFd<'_>, enable: bool, idle: i32) -> Result<()> {
	let val = enable as i32;

	setsockopt(
		socket,
		SocketLevel::Socket as i32,
		SocketOption::KeepAlive as i32,
		&val
	)?;

	if enable {
		setsockopt(
			socket,
			SocketLevel::Tcp as i32,
			TcpOption::KeepIdle as i32,
			&idle
		)?;
	}

	Ok(())
}
