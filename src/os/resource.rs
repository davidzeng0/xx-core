use std::{io::Result, mem::zeroed};

use super::{
	syscall::{syscall_int, to_pointer, SyscallNumber::*},
	time::TimeVal
};

pub enum Resource {
	///Per-process CPU limit, in seconds.
	Cpu,

	///Largest file that can be created, in bytes.
	FSize,

	///Maximum size of data segment, in bytes.
	Data,

	///Maximum size of stack segment, in bytes.
	Stack,

	///Largest core file that can be created, in bytes.
	Core,

	/// Largest resident set size, in bytes.
	/// This affects swapping; processes that are exceeding their
	/// resident set size will be more likely to have physical memory
	/// taken from them.
	Rss,

	///Number of processes.
	NProc,

	///Number of open files.
	NoFile,

	///Locked-in-memory address space.
	MemLock,

	///Address space limit.
	As,

	///Maximum number of file locks.
	Locks,

	///Maximum number of pending signals.
	SigPending,

	///Maximum bytes in POSIX message queues.
	MsgQueue,

	/// Maximum nice priority allowed to raise to.
	/// Nice levels 19 .. -20 correspond to 0 .. 39
	/// values of this resource limit.
	Nice,

	///Maximum realtime priority allowed for non-priviledged processes.
	RtPrio,

	/// Maximum CPU time in microseconds that a process scheduled under a
	/// real-time scheduling policy may consume without making a blocking system
	/// call before being forcibly descheduled.
	RtTime
}

#[repr(u64)]
pub enum Constant {
	Infinity = u64::MAX
}

#[repr(C)]
pub struct Limit {
	current: u64,
	maximum: u64
}

impl Limit {
	fn new() -> Limit {
		Limit { current: 0, maximum: 0 }
	}
}

pub enum UsageWho {
	/// The calling process. (renamed from Self)
	Current  = 0,

	/// All of its terminated child processes.
	Children = -1,

	/// The calling thread.
	Thread   = 1
}

pub enum PriorityWhich {
	/// WHO is a process ID.
	Process,

	/// WHO is a process group ID.
	PGrp,

	/// WHO is a user ID.
	User
}

pub struct Usage {
	pub user_time: TimeVal,
	pub sys_time: TimeVal,
	pub max_rss: i64,
	pub text_rss: i64,
	pub data_rss: i64,
	pub stack_rss: i64,
	pub minor_flt: i64,
	pub major_fault: i64,
	pub swaps: i64,
	pub input_block: i64,
	pub output_block: i64,
	pub ipc_msgs_sent: i64,
	pub ipc_msgs_recvd: i64,
	pub signals_delivered: i64,
	pub voluntary_context_switches: i64,
	pub involuntary_context_switches: i64
}

impl Usage {
	fn new() -> Usage {
		unsafe { zeroed() }
	}
}

pub fn get_rlimit(resource: Resource) -> Result<Limit> {
	let mut limit = Limit::new();

	syscall_int!(Getrlimit, resource, to_pointer(&mut limit))?;

	Ok(limit)
}

pub fn set_rlimit(resource: Resource, limit: &Limit) -> Result<()> {
	syscall_int!(Setrlimit, resource, to_pointer(limit))?;

	Ok(())
}

pub fn p_rlimit(pid: Option<i32>, resource: Resource, new_limit: Option<&Limit>) -> Result<Limit> {
	let mut limit = Limit::new();

	syscall_int!(
		Prlimit64,
		pid.unwrap_or(0),
		resource,
		new_limit.map_or(0, |rlimit| { to_pointer(rlimit) }),
		to_pointer(&mut limit)
	)?;

	Ok(limit)
}

pub fn get_limit(resource: Resource) -> Result<u64> {
	Ok(get_rlimit(resource)?.current)
}

pub fn get_rusage(who: UsageWho) -> Result<Usage> {
	let mut usage = Usage::new();

	syscall_int!(Getrusage, who, to_pointer(&mut usage))?;

	Ok(usage)
}

pub fn get_priority(which: PriorityWhich, who: Option<u32>) -> Result<i32> {
	const PRIORITY_ZERO: i32 = 20;
	let prio = syscall_int!(Getpriority, which, who.unwrap_or(0))?;

	Ok(PRIORITY_ZERO - prio as i32)
}

pub fn set_priority(which: PriorityWhich, who: Option<u32>, prio: i32) -> Result<()> {
	syscall_int!(Setpriority, which, who.unwrap_or(0), prio)?;

	Ok(())
}
