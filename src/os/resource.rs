use super::{time::TimeVal, *};

define_enum! {
	#[repr(u32)]
	pub enum Resource {
		/// Per-process CPU limit, in seconds.
		Cpu,

		/// Largest file that can be created, in bytes.
		FSize,

		/// Maximum size of data segment, in bytes.
		Data,

		/// Maximum size of stack segment, in bytes.
		Stack,

		/// Largest core file that can be created, in bytes.
		Core,

		/// Largest resident set size, in bytes.
		/// This affects swapping; processes that are exceeding their
		/// resident set size will be more likely to have physical memory
		/// taken from them.
		Rss,

		/// Number of processes.
		NProc,

		/// Number of open files.
		NoFile,

		/// Locked-in-memory address space.
		MemLock,

		/// Address space limit.
		As,

		/// Maximum number of file locks.
		Locks,

		/// Maximum number of pending signals.
		SigPending,

		/// Maximum bytes in POSIX message queues.
		MsgQueue,

		/// Maximum nice priority allowed to raise to.
		/// Nice levels 19 .. -20 correspond to 0 .. 39
		/// values of this resource limit.
		Nice,

		/// Maximum realtime priority allowed for non-priviledged processes.
		RtPrio,

		/// Maximum CPU time in microseconds that a process scheduled under a
		/// real-time scheduling policy may consume without making a blocking system
		/// call before being forcibly descheduled.
		RtTime
	}
}

pub const UNLIMITED: u64 = u64::MAX;

define_struct! {
	pub struct Limit {
		pub current: u64,
		pub maximum: u64
	}
}

define_enum! {
	#[repr(i32)]
	pub enum UsageWho {
		/// The calling process. (renamed from Self)
		Process  = 0,

		/// All of its terminated child processes.
		Children = -1,

		/// The calling thread.
		Thread   = 1
	}
}

define_enum! {
	#[repr(u32)]
	pub enum PriorityWhich {
		/// WHO is a process ID.
		Process,

		/// WHO is a process group ID.
		PGrp,

		/// WHO is a user ID.
		User
	}
}

define_struct! {
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
}

pub fn get_rlimit(resource: Resource) -> Result<Limit> {
	let mut limit = Limit::default();

	unsafe { syscall_int!(Getrlimit, resource as u32, &mut limit)? };

	Ok(limit)
}

pub fn set_rlimit(resource: Resource, limit: &Limit) -> Result<()> {
	unsafe { syscall_int!(Setrlimit, resource as u32, limit)? };

	Ok(())
}

pub fn p_rlimit(pid: Option<i32>, resource: Resource, new_limit: Option<&Limit>) -> Result<Limit> {
	let mut limit = Limit::default();

	unsafe {
		syscall_int!(
			Prlimit64,
			pid.unwrap_or(0),
			resource as u32,
			new_limit.map_or(Ptr::null(), |rlimit| { Ptr::from(rlimit) }),
			&mut limit
		)?
	};

	Ok(limit)
}

pub fn get_limit(resource: Resource) -> Result<u64> {
	Ok(get_rlimit(resource)?.current)
}

pub fn get_rusage(who: UsageWho) -> Result<Usage> {
	let mut usage = Usage::default();

	unsafe { syscall_int!(Getrusage, who as i32, &mut usage)? };

	Ok(usage)
}

pub fn get_priority(which: PriorityWhich, who: Option<u32>) -> Result<i32> {
	const PRIORITY_ZERO: i32 = 20;
	let prio = unsafe { syscall_int!(Getpriority, which as u32, who.unwrap_or(0))? };

	Ok(PRIORITY_ZERO - prio as i32)
}

pub fn set_priority(which: PriorityWhich, who: Option<u32>, prio: i32) -> Result<()> {
	unsafe { syscall_int!(Setpriority, which as u32, who.unwrap_or(0), prio)? };

	Ok(())
}
