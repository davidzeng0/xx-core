#![allow(clippy::module_name_repetitions)]

use super::{
	fcntl::OpenFlag,
	inet::Address,
	iovec::{self, IoVec, IoVecMut},
	tcp::TcpOption,
	*
};

define_enum! {
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
}

#[allow(non_upper_case_globals)]
impl ProtocolFamily {
	/// Another non-standard name for PF_LOCAL.
	pub const File: Self = Self::Local;
	/// Alias to emulate 4.4BSD.
	pub const Route: Self = Self::NetLink;
	/// POSIX name for PF_LOCAL.
	pub const Unix: Self = Self::Local;
}

pub type AddressFamily = ProtocolFamily;

define_enum! {
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
}
define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum SocketFlag {
		CloseOnExec = OpenFlag::CloseOnExec as u32,
		NonBlock    = OpenFlag::NonBlock as u32
	}
}

define_enum! {
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
}

define_enum! {
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
}

define_enum! {
	#[repr(u32)]
	pub enum Shutdown {
		Read,
		Write,
		Both
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
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
}

pub mod raw {
	use iovec::raw::IoVec;

	use super::*;

	define_struct! {
		pub struct MsgHdr {
			pub address: MutPtr<()>,
			pub address_len: u32,

			pub iov: MutPtr<IoVec>,
			pub iov_len: usize,

			pub control: Ptr<()>,
			pub control_len: usize,

			pub flags: u32
		}
	}

	impl MsgHdr {
		#[must_use]
		pub fn flags(&self) -> BitFlags<MessageFlag> {
			BitFlags::from_bits_truncate(self.flags)
		}
	}

	#[repr(transparent)]
	#[derive(Default, Debug)]
	pub struct BorrowedMsgHdr<'a, const MUTABLE: bool> {
		pub msg_hdr: MsgHdr,
		pub phantom: PhantomData<&'a ()>
	}

	#[derive(Default, Debug)]
	pub struct ExtraBuf<'a, const MUTABLE: bool> {
		pub ptr: MutPtr<()>,
		pub len: i32,
		pub phantom: PhantomData<&'a ()>
	}
}

pub type MsgHdr<'a> = raw::BorrowedMsgHdr<'a, false>;
pub type MsgHdrMut<'a> = raw::BorrowedMsgHdr<'a, true>;

impl<const MUTABLE: bool> raw::BorrowedMsgHdr<'_, MUTABLE> {
	#[must_use]
	pub fn flags(&self) -> BitFlags<MessageFlag> {
		self.msg_hdr.flags()
	}
}

impl<'a> MsgHdr<'a> {
	/// # Panics
	/// if size of A cannot fit in an i32
	#[allow(clippy::unwrap_used)]
	pub fn set_addr<A>(&mut self, addr: &'a A) {
		self.msg_hdr.address = Ptr::from(addr).cast_mut().as_unit();
		self.msg_hdr.address_len = size_of::<A>().try_into().unwrap();
	}

	pub fn set_vecs<'b, 'c>(&mut self, vecs: &'b [IoVec<'c>])
	where
		'b: 'a,
		'c: 'a
	{
		self.msg_hdr.iov = Ptr::from(vecs.as_ptr()).cast_mut().cast();
		self.msg_hdr.iov_len = vecs.len();
	}
}

impl<'a> MsgHdrMut<'a> {
	/// # Panics
	/// if size of A cannot fit in an i32
	#[allow(clippy::unwrap_used)]
	pub fn set_addr<A>(&mut self, addr: &'a mut A) {
		self.msg_hdr.address = MutPtr::from(addr).as_unit();
		self.msg_hdr.address_len = size_of::<A>().try_into().unwrap();
	}

	pub fn set_vecs<'b, 'c>(&mut self, vecs: &'b mut [IoVecMut<'c>])
	where
		'b: 'a,
		'c: 'a
	{
		self.msg_hdr.iov = MutPtr::from(vecs.as_mut_ptr()).cast();
		self.msg_hdr.iov_len = vecs.len();
	}
}

pub type ExtraBuf<'a> = raw::ExtraBuf<'a, false>;
pub type ExtraBufMut<'a> = raw::ExtraBuf<'a, true>;

impl ExtraBuf<'_> {
	#[must_use]
	pub const fn from_parts(ptr: Ptr<()>, len: i32) -> Self {
		Self { ptr: ptr.cast_mut(), len, phantom: PhantomData }
	}
}

impl ExtraBufMut<'_> {
	#[must_use]
	pub const fn from_parts(ptr: MutPtr<()>, len: i32) -> Self {
		Self { ptr, len, phantom: PhantomData }
	}
}

impl<'a, T> From<&'a T> for ExtraBuf<'a> {
	/// # Panics
	/// if size of A cannot fit in an i32
	#[allow(clippy::unwrap_used)]
	fn from(value: &'a T) -> Self {
		Self {
			ptr: Ptr::from(value).cast_mut().cast(),
			len: size_of::<T>().try_into().unwrap(),
			phantom: PhantomData
		}
	}
}

impl<'a, T> From<&'a mut T> for ExtraBufMut<'a> {
	/// # Panics
	/// if size of A cannot fit in an i32
	#[allow(clippy::unwrap_used)]
	fn from(value: &'a mut T) -> Self {
		Self {
			ptr: MutPtr::from(value).cast(),
			len: size_of::<T>().try_into().unwrap(),
			phantom: PhantomData
		}
	}
}

impl IntoRawArray for ExtraBuf<'_> {
	type Length = i32;
	type Pointer = Ptr<()>;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length) {
		(self.ptr.cast_const(), self.len)
	}
}

impl<'a> IntoRawArray for &'a mut ExtraBufMut<'_> {
	type Length = &'a mut i32;
	type Pointer = MutPtr<()>;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length) {
		(self.ptr, &mut self.len)
	}
}

impl IntoRawArray for Option<&mut ExtraBufMut<'_>> {
	type Length = MutPtr<i32>;
	type Pointer = MutPtr<()>;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length) {
		self.map_or_else(Default::default, |buf| {
			let raw = buf.into_raw_array();

			(raw.0, MutPtr::from(raw.1))
		})
	}
}

/// Note: changed i32 to u32 as negative values aren't allowed anyway
#[syscall_define(Socket)]
pub fn socket(domain: u32, socket_type: u32, protocol: u32) -> Result<OwnedFd>;

/// # Safety
/// `addr` must be a valid buffer
#[syscall_define(Bind)]
pub unsafe fn bind(socket: BorrowedFd<'_>, #[array] addr: ExtraBuf<'_>) -> Result<()>;

pub fn bind_addr(socket: BorrowedFd<'_>, addr: &Address) -> Result<()> {
	/* Safety: addr is a valid reference */
	#[allow(clippy::multiple_unsafe_ops_per_block)]
	unsafe {
		match &addr {
			Address::V4(addr) => bind(socket, addr.into()),
			Address::V6(addr) => bind(socket, addr.into())
		}
	}
}

/// # Safety
/// `addr` must be a valid buffer
#[syscall_define(Connect)]
pub unsafe fn connect(socket: BorrowedFd<'_>, #[array] addr: ExtraBuf<'_>) -> Result<()>;

pub fn connect_addr(socket: BorrowedFd<'_>, addr: &Address) -> Result<()> {
	/* Safety: addr is a valid reference */
	#[allow(clippy::multiple_unsafe_ops_per_block)]
	unsafe {
		match &addr {
			Address::V4(addr) => connect(socket, addr.into()),
			Address::V6(addr) => connect(socket, addr.into())
		}
	}
}

/// # Safety
/// `addr` must be a valid buffer
#[syscall_define(Accept)]
pub unsafe fn accept(
	socket: BorrowedFd<'_>, #[array] addr: Option<&mut ExtraBufMut<'_>>
) -> Result<OwnedFd>;

#[syscall_define(Accept4)]
pub unsafe fn accept4(
	socket: BorrowedFd<'_>, #[array] addr: Option<&mut ExtraBufMut<'_>>,
	flags: BitFlags<SocketFlag>
) -> Result<OwnedFd>;

pub fn accept_storage<A>(socket: BorrowedFd<'_>, addr: &mut A) -> Result<(OwnedFd, i32)> {
	let mut buf = ExtraBufMut::from(addr);

	/* Safety: buf is from a valid reference */
	let fd = unsafe { accept(socket, Some(&mut buf))? };

	Ok((fd, buf.len))
}

/// # Safety
/// `addr` must be a valid buffer
#[syscall_define(Sendto)]
pub unsafe fn sendto(
	socket: BorrowedFd<'_>, #[array] buf: RawBuf<'_>, flags: BitFlags<MessageFlag>,
	#[array] destination: ExtraBuf<'_>
) -> Result<usize>;

/// # Safety
/// `buf` and `addr` must be a valid buffer
pub unsafe fn sendto_arbitrary<A>(
	socket: BorrowedFd<'_>, buf: RawBuf<'_>, flags: BitFlags<MessageFlag>, destination: &A
) -> Result<usize> {
	/* Safety: guaranteed by caller */
	unsafe { sendto(socket, buf, flags, ExtraBuf::from(destination)) }
}

/// # Safety
/// `buf` and `addr` must be a valid buffer
pub unsafe fn send(
	socket: BorrowedFd<'_>, buf: RawBuf<'_>, flags: BitFlags<MessageFlag>
) -> Result<usize> {
	/* Safety: guaranteed by caller */
	unsafe { sendto(socket, buf, flags, ExtraBuf::default()) }
}

/// # Safety
/// `buf` and `addr` must be a valid buffer
#[syscall_define(Recvfrom)]
pub unsafe fn recvfrom(
	socket: BorrowedFd<'_>, #[array] buf: RawBuf<'_>, flags: BitFlags<MessageFlag>,
	#[array] addr: Option<&mut ExtraBufMut<'_>>
) -> Result<usize>;

/// # Safety
/// `buf`, and `len` must be valid
pub unsafe fn recvfrom_arbitrary<A>(
	socket: BorrowedFd<'_>, buf: RawBuf<'_>, flags: BitFlags<MessageFlag>, addr: &mut A
) -> Result<(usize, i32)> {
	let mut addr_buf = ExtraBufMut::from(addr);

	/* Safety: guaranteed by caller */
	let recvd = unsafe { recvfrom(socket, buf, flags, Some(&mut addr_buf))? };

	Ok((recvd, addr_buf.len))
}

/// # Safety
/// `buf` must be a valid buffer
pub unsafe fn recv(
	socket: BorrowedFd<'_>, buf: RawBuf<'_>, flags: BitFlags<MessageFlag>
) -> Result<usize> {
	/* Safety: guaranteed by caller */
	unsafe { recvfrom(socket, buf, flags, None) }
}

pub const MAX_BACKLOG: i32 = 4096;

#[syscall_define(Listen)]
pub fn listen(socket: BorrowedFd<'_>, backlog: i32) -> Result<()>;

#[syscall_define(Shutdown)]
pub fn shutdown(socket: BorrowedFd<'_>, how: Shutdown) -> Result<()>;

#[syscall_define(Getsockopt)]
pub unsafe fn getsockopt(
	socket: BorrowedFd<'_>, level: i32, option: i32, #[array] val: &mut ExtraBufMut<'_>
) -> Result<u32>;

pub fn getsockopt_arbitrary<Opt>(
	socket: BorrowedFd<'_>, level: i32, option: i32, opt_val: &mut Opt
) -> Result<(u32, i32)> {
	let mut opt_buf = ExtraBufMut::from(opt_val);

	/* Safety: opt_buf was constructed from a valid reference */
	let ret = unsafe { getsockopt(socket, level, option, &mut opt_buf) }?;

	Ok((ret, opt_buf.len))
}

#[syscall_define(Setsockopt)]
pub unsafe fn setsockopt(
	socket: BorrowedFd<'_>, level: i32, option: i32, #[array] val: ExtraBuf<'_>
) -> Result<u32>;

pub fn setsockopt_arbitrary<Opt>(
	socket: BorrowedFd<'_>, level: i32, option: i32, opt_val: &Opt
) -> Result<u32> {
	/* Safety: opt_val is constructed from a valid reference */
	unsafe { setsockopt(socket, level, option, ExtraBuf::from(opt_val)) }
}

pub fn set_reuse_addr(socket: BorrowedFd<'_>, enable: bool) -> Result<()> {
	let enable = enable as i32;

	setsockopt_arbitrary(
		socket,
		SocketLevel::Socket as i32,
		SocketOption::ReuseAddr as i32,
		&enable
	)?;

	Ok(())
}

pub fn set_recvbuf_size(socket: BorrowedFd<'_>, size: i32) -> Result<()> {
	setsockopt_arbitrary(
		socket,
		SocketLevel::Socket as i32,
		SocketOption::RecvBufSize as i32,
		&size
	)?;

	Ok(())
}

pub fn set_sendbuf_size(socket: BorrowedFd<'_>, size: i32) -> Result<()> {
	setsockopt_arbitrary(
		socket,
		SocketLevel::Socket as i32,
		SocketOption::SendBufSize as i32,
		&size
	)?;

	Ok(())
}

pub fn set_tcp_nodelay(socket: BorrowedFd<'_>, enable: bool) -> Result<()> {
	let enable = enable as i32;

	setsockopt_arbitrary(
		socket,
		SocketLevel::Tcp as i32,
		TcpOption::NoDelay as i32,
		&enable
	)?;

	Ok(())
}

pub fn set_tcp_keepalive(socket: BorrowedFd<'_>, enable: bool, idle: i32) -> Result<()> {
	let val = enable as i32;

	setsockopt_arbitrary(
		socket,
		SocketLevel::Socket as i32,
		SocketOption::KeepAlive as i32,
		&val
	)?;

	if enable {
		setsockopt_arbitrary(
			socket,
			SocketLevel::Tcp as i32,
			TcpOption::KeepIdle as i32,
			&idle
		)?;
	}

	Ok(())
}

/// # Safety
/// `addr` must be a valid buffer
#[syscall_define(Getsockname)]
pub unsafe fn getsockname(
	socket: BorrowedFd<'_>, #[array] addr: &mut ExtraBufMut<'_>
) -> Result<()>;

/// # Safety
/// `addr` must be a valid buffer
#[syscall_define(Getpeername)]
pub unsafe fn getpeername(
	socket: BorrowedFd<'_>, #[array] addr: &mut ExtraBufMut<'_>
) -> Result<()>;

/// # Safety
/// `func` must be a valid getaddr function
unsafe fn get_addr<A>(
	func: unsafe fn(BorrowedFd<'_>, &mut ExtraBufMut<'_>) -> Result<()>, socket: BorrowedFd<'_>,
	addr: &mut A
) -> Result<i32> {
	let mut buf = ExtraBufMut::from(addr);

	/* Safety: guaranteed by caller */
	unsafe { func(socket, &mut buf)? };

	Ok(buf.len)
}

pub fn get_sock_name<A>(socket: BorrowedFd<'_>, addr: &mut A) -> Result<i32> {
	/* Safety: getsockname is valid */
	unsafe { get_addr(getsockname, socket, addr) }
}

pub fn get_peer_name<A>(socket: BorrowedFd<'_>, addr: &mut A) -> Result<i32> {
	/* Safety: getsockname is valid */
	unsafe { get_addr(getpeername, socket, addr) }
}
