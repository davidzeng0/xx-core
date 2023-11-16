use std::{
	mem::zeroed,
	net::{IpAddr, SocketAddr, SocketAddrV6},
	ptr::copy
};

use super::socket::AddressFamily;
use crate::pointer::*;

#[repr(u32)]
pub enum IpProtocol {
	/// Dummy protocol for TCP.
	Ip       = 1,

	/// Internet Control Message Protocol.
	Icmp     = 2,

	/// Internet Group Management Protocol.
	Igmp     = 3,

	/// IPIP tunnels (older KA9Q tunnels use 94).
	Ipip     = 4,

	/// Transmission Control Protocol.
	Tcp      = 6,

	/// Exterior Gateway Protocol.
	Egp      = 8,

	/// PUP protocol.
	Pup      = 12,

	/// User Datagram Protocol.
	Udp      = 17,

	/// XNS IDP protocol.
	Idp      = 22,

	/// SO Transport Protocol Class 4.
	Tp       = 29,

	/// Datagram Congestion Control Protocol.
	Dccp     = 33,

	/// IPv6 header.
	Ipv6     = 41,

	/// Reservation Protocol.
	Rsvp     = 46,

	/// General Routing Encapsulation.
	Gre      = 47,

	/// encapsulating security payload.
	Esp      = 50,

	/// authentication header.
	Ah       = 51,

	/// Multicast Transport Protocol.
	Mtp      = 92,

	/// IP option pseudo header for BEET.
	Beetph   = 94,

	/// Encapsulation Header.
	Encap    = 98,

	/// Protocol Independent Multicast.
	Pim      = 103,

	/// Compression Header Protocol.
	Comp     = 108,

	/// Stream Control Transmission Protocol.
	Sctp     = 132,

	/// UDP-Lite protocol.
	Udplite  = 136,

	/// MPLS in IP.
	Mpls     = 137,

	/// Ethernet-within-IPv6 Encapsulation.
	Ethernet = 143,

	/// Raw IP packets.
	Raw      = 255,

	/// Multipath TCP connection.
	Mptcp    = 262
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct AddressCommon {
	pub family: u16
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct AddressV4 {
	pub common: AddressCommon,
	pub port: u16,
	pub addr: [u8; 4],
	pub pad: [u8; 8]
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct AddressV6 {
	pub common: AddressCommon,
	pub port: u16,
	pub flow_info: u32,
	pub addr: [u8; 16],
	pub scope_id: u32
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct AddressStorage {
	pub common: AddressCommon,
	pub pad: [u64; 15]
}

impl AddressStorage {
	pub fn new() -> Self {
		unsafe { zeroed() }
	}
}

pub enum Address {
	V4(AddressV4),
	V6(AddressV6)
}

impl From<SocketAddr> for Address {
	fn from(value: SocketAddr) -> Self {
		match value {
			SocketAddr::V4(addr) => Address::V4(AddressV4 {
				common: AddressCommon { family: AddressFamily::INet as u16 },
				port: addr.port().to_be(),
				addr: addr.ip().octets(),
				pad: [0u8; 8]
			}),

			SocketAddr::V6(addr) => Address::V6(AddressV6 {
				common: AddressCommon { family: AddressFamily::INet6 as u16 },
				port: addr.port().to_be(),
				flow_info: addr.flowinfo().to_be(),
				addr: addr.ip().octets(),
				scope_id: addr.scope_id().to_be()
			})
		}
	}
}

impl TryFrom<AddressStorage> for Address {
	type Error = ();

	fn try_from(value: AddressStorage) -> Result<Self, ()> {
		const INET: u16 = AddressFamily::INet as u16;
		const INET6: u16 = AddressFamily::INet6 as u16;

		match value.common.family {
			INET => {
				let addr = unsafe {
					let mut addr: AddressV4 = zeroed();
					let ptr = Ptr::from(&value).cast::<AddressV4>();

					copy(ptr.as_ptr(), &mut addr, 1);

					addr
				};

				Ok(Address::V4(addr))
			}

			INET6 => {
				let addr = unsafe {
					let mut addr: AddressV6 = zeroed();
					let ptr = Ptr::from(&value).cast::<AddressV6>();

					copy(ptr.as_ptr(), &mut addr, 1);

					addr
				};

				Ok(Address::V6(addr))
			}

			_ => Err(())
		}
	}
}

impl TryFrom<AddressStorage> for SocketAddr {
	type Error = ();

	fn try_from(value: AddressStorage) -> Result<Self, ()> {
		let addr: Address = value.try_into()?;

		Ok(match addr {
			Address::V4(addr) => SocketAddr::new(IpAddr::V4(addr.addr.into()), addr.port.to_be()),
			Address::V6(addr) => SocketAddr::V6(SocketAddrV6::new(
				addr.addr.into(),
				addr.port.to_be(),
				addr.flow_info.to_be(),
				addr.scope_id.to_be()
			))
		})
	}
}
