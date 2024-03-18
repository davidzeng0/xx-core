#![allow(clippy::module_name_repetitions)]

use super::*;

pub type SignalSet = u64;

define_enum! {
	pub enum Signal {
		/// Hang up controlling terminal or process
		Hangup = 1,

		/// Interrupt from keyboard, ^C
		Interrupt,

		/// Quit from keyboard, ^\
		Quit,

		IllegalInstruction,

		/// Breakpoint for debugging
		Trap,

		/// Abnormal termination
		Abort,

		/// Bus error
		Bus,

		FloatingPointException,

		/// Forced process termination
		Kill,

		/// User-defined signal 1
		User1,

		/// Invalid memory reference
		SegmentationFault,

		/// User-defined signal 2
		User2,

		/// Write to pipe with no readers
		Pipe,

		/// Real-timer clock
		Alarm,

		/// Process termination
		Termination,

		/// Stack fault (obsolete)
		StackFault,

		/// Child process terminated or stopped
		Child,

		/// Resume execution, if stopped
		Continue,

		/// Stop process execution, ^Z
		Stop,

		/// Keyboard stop
		TTYStop,

		/// Background read from control terminal
		TTIn,

		/// Background write to control terminal
		TTOut,

		/// Urgent condition on socket
		Urgent,

		/// CPU time limit exceeded
		ExceedCpu,

		/// File size limit exceeded
		ExceedFileSize,

		/// Virtual timer expired
		VirtualAlarm,

		/// Profiling timer expired
		Profile,

		/// Window size change
		WindowChange,

		/// I/O now possible
		Io,

		/// Power failure imminent
		Power,

		/// Bad system call
		Syscall
	}
}

#[allow(non_upper_case_globals)]
impl Signal {
	/// IOT instruction, abort
	pub const Iot: Self = Self::Abort;
	/// Equivalent to Io
	pub const Poll: Self = Self::Io;
	/// Equivalent to Syscall
	pub const Unused: Self = Self::Syscall;
}

pub const SIGRTMAX: u32 = 65;

pub type SignalMask<'a> = Option<&'a [SignalSet]>;

impl<'a> From<SignalMask<'a>> for RawBuf<'a> {
	fn from(value: SignalMask<'a>) -> Self {
		let parts = value.into_raw_array();

		Self::from_parts(parts.0.cast(), parts.1)
	}
}
