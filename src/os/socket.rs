use std::{
	mem::{size_of, zeroed},
	os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd}
};

use enumflags2::{bitflags, BitFlags};

use super::{
	inet::Address,
	iovec::IoVec,
	syscall::{
		syscall_int,
		SyscallNumber::{self, *}
	},
	tcp::TcpOption
};
use crate::{error::Result, pointer::*};

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
pub struct MessageHeader<'a> {
	pub address: MutPtr<()>,
	pub address_len: u32,

	pub iov: MutPtr<IoVec<'a>>,
	pub iov_len: usize,

	pub control: Ptr<()>,
	pub control_len: usize,

	pub flags: u32
}

impl MessageHeader<'_> {
	pub fn new() -> Self {
		unsafe { zeroed() }
	}

	pub fn set_addr<A>(&mut self, addr: &mut A) {
		self.address = MutPtr::from(addr).as_unit();
		self.address_len = size_of::<A>() as u32;
	}

	pub fn flags(&self) -> BitFlags<MessageFlag> {
		unsafe { BitFlags::from_bits_unchecked(self.flags) }
	}
}

impl<'a> MessageHeader<'a> {
	pub fn set_vecs<'b: 'a>(&mut self, vecs: &mut [IoVec<'b>]) {
		self.iov = MutPtr::from(vecs.as_mut_ptr());
		self.iov_len = vecs.len();
	}
}

pub fn socket(domain: u32, socket_type: u32, protocol: u32) -> Result<OwnedFd> {
	unsafe {
		let fd = syscall_int!(Socket, domain, socket_type, protocol)?;

		Ok(OwnedFd::from_raw_fd(fd as i32))
	}
}

pub unsafe fn bind_raw(socket: BorrowedFd<'_>, addr: Ptr<()>, addrlen: u32) -> Result<()> {
	syscall_int!(Bind, socket.as_raw_fd(), addr.int_addr(), addrlen)?;

	Ok(())
}

pub fn bind<A>(socket: BorrowedFd<'_>, addr: &A) -> Result<()> {
	unsafe { bind_raw(socket, Ptr::from(addr).as_unit(), size_of::<A>() as u32) }
}

pub fn bind_addr(socket: BorrowedFd<'_>, addr: &Address) -> Result<()> {
	match &addr {
		Address::V4(addr) => bind(socket, addr),
		Address::V6(addr) => bind(socket, addr)
	}
}

pub unsafe fn connect_raw(socket: BorrowedFd<'_>, addr: Ptr<()>, addrlen: u32) -> Result<()> {
	syscall_int!(Connect, socket.as_raw_fd(), addr.int_addr(), addrlen)?;

	Ok(())
}

pub fn connect<A>(socket: BorrowedFd<'_>, addr: &A) -> Result<()> {
	unsafe { connect_raw(socket, Ptr::from(addr).as_unit(), size_of::<A>() as u32) }
}

pub fn connect_addr(socket: BorrowedFd<'_>, addr: &Address) -> Result<()> {
	match &addr {
		Address::V4(addr) => connect(socket, addr),
		Address::V6(addr) => connect(socket, addr)
	}
}

pub unsafe fn accept_raw(
	socket: BorrowedFd<'_>, addr: MutPtr<()>, addrlen: &mut u32
) -> Result<OwnedFd> {
	let fd = syscall_int!(
		Accept,
		socket.as_raw_fd(),
		addr.int_addr(),
		MutPtr::from(addrlen).int_addr()
	)?;

	Ok(OwnedFd::from_raw_fd(fd as i32))
}

pub fn accept<A>(socket: BorrowedFd<'_>, addr: &mut A) -> Result<(OwnedFd, u32)> {
	let mut addrlen = size_of::<A>() as u32;
	let fd = unsafe { accept_raw(socket, MutPtr::from(addr).as_unit(), &mut addrlen)? };

	Ok((fd, addrlen))
}

pub unsafe fn sendto_raw(
	socket: BorrowedFd<'_>, buf: Ptr<()>, len: usize, flags: u32, dest_addr: Ptr<()>, addrlen: u32
) -> Result<usize> {
	let sent = syscall_int!(
		Sendto,
		socket.as_raw_fd(),
		buf.int_addr(),
		len,
		flags,
		dest_addr.int_addr(),
		addrlen
	)?;

	Ok(sent as usize)
}

pub unsafe fn sendto<A>(
	socket: BorrowedFd<'_>, buf: Ptr<()>, len: usize, flags: u32, destination: &A
) -> Result<usize> {
	sendto_raw(
		socket,
		buf,
		len,
		flags,
		Ptr::from(destination).as_unit(),
		size_of::<A>() as u32
	)
}

pub unsafe fn send(socket: BorrowedFd<'_>, buf: Ptr<()>, len: usize, flags: u32) -> Result<usize> {
	sendto_raw(socket, buf, len, flags, Ptr::null(), 0)
}

pub unsafe fn recvfrom_raw(
	socket: BorrowedFd<'_>, buf: Ptr<()>, len: usize, flags: u32, addr: MutPtr<()>,
	addrlen: &mut u32
) -> Result<usize> {
	let received = syscall_int!(
		Recvfrom,
		socket.as_raw_fd(),
		buf.int_addr(),
		len,
		flags,
		addr.int_addr(),
		MutPtr::from(addrlen).int_addr()
	)?;

	Ok(received as usize)
}

pub unsafe fn recvfrom<A>(
	socket: BorrowedFd<'_>, buf: Ptr<()>, len: usize, flags: u32, addr: &mut A
) -> Result<(usize, u32)> {
	let mut addrlen = size_of::<A>() as u32;
	let recvd = recvfrom_raw(
		socket,
		buf,
		len,
		flags,
		MutPtr::from(addr).as_unit(),
		&mut addrlen
	)?;

	Ok((recvd, addrlen))
}

pub unsafe fn recv(socket: BorrowedFd<'_>, buf: Ptr<()>, len: usize, flags: u32) -> Result<usize> {
	let received = syscall_int!(
		Recvfrom,
		socket.as_raw_fd(),
		buf.int_addr(),
		len,
		flags,
		0,
		0
	)?;

	Ok(received as usize)
}

pub const MAX_BACKLOG: i32 = 4096;

pub fn listen(socket: BorrowedFd<'_>, backlog: i32) -> Result<()> {
	unsafe { syscall_int!(Listen, socket.as_raw_fd(), backlog)? };

	Ok(())
}

pub fn shutdown(socket: BorrowedFd<'_>, how: Shutdown) -> Result<()> {
	unsafe { syscall_int!(Shutdown, socket.as_raw_fd(), how)? };

	Ok(())
}

pub fn getsockopt<Opt>(
	socket: BorrowedFd<'_>, level: i32, option: i32, opt_val: &mut Opt
) -> Result<u32> {
	let res = unsafe {
		syscall_int!(
			Setsockopt,
			socket.as_raw_fd(),
			level,
			option,
			MutPtr::from(opt_val).int_addr(),
			size_of::<Opt>
		)?
	};

	Ok(res as u32)
}

pub fn setsockopt<Opt>(
	socket: BorrowedFd<'_>, level: i32, option: i32, opt_val: &Opt
) -> Result<u32> {
	let res = unsafe {
		syscall_int!(
			Setsockopt,
			socket.as_raw_fd(),
			level,
			option,
			Ptr::from(opt_val).int_addr(),
			size_of::<Opt>()
		)?
	};

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

pub unsafe fn get_addr_raw(
	number: SyscallNumber, socket: BorrowedFd<'_>, addr: MutPtr<()>, addrlen: &mut u32
) -> Result<()> {
	syscall_int!(
		number,
		socket.as_raw_fd(),
		addr.int_addr(),
		MutPtr::from(addrlen).int_addr()
	)?;

	Ok(())
}

pub fn get_addr<A>(number: SyscallNumber, socket: BorrowedFd<'_>, addr: &mut A) -> Result<u32> {
	let mut addrlen = size_of::<A>() as u32;

	unsafe { get_addr_raw(number, socket, MutPtr::from(addr).as_unit(), &mut addrlen)? };

	Ok(addrlen)
}

pub fn get_sock_name<A>(socket: BorrowedFd<'_>, addr: &mut A) -> Result<u32> {
	get_addr(Getsockname, socket, addr)
}

pub fn get_peer_name<A>(socket: BorrowedFd<'_>, addr: &mut A) -> Result<u32> {
	get_addr(Getpeername, socket, addr)
}
