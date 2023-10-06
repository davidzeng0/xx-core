use std::io::Result;

use super::error::result_from_libc;

#[repr(C)]
pub struct TimeVal {
	pub sec: i64,
	pub micros: i64
}

#[repr(C)]
pub struct TimeSpec {
	pub sec: i64,
	pub nanos: i64
}

#[repr(C)]
pub enum ClockId {
	RealTime,
	Monotonic,
	ProcessCpuTimeId,
	ThreadCpuTimeId,
	MonotonicRaw,
	RealTimeCoarse,
	MonotonicCoarse,
	BootTime,
	RealTimeAlarm,
	BootTimeAlarm,
	Tai = 11
}

extern "C" {
	fn clock_gettime(clock: ClockId, spec: &mut TimeSpec) -> i32;
}

pub fn time(clock: ClockId) -> Result<u64> {
	let mut ts = TimeSpec { sec: 0, nanos: 0 };

	result_from_libc(unsafe { clock_gettime(clock, &mut ts) } as isize)?;

	Ok((ts.sec as u64) * 1_000_000_000 + ts.nanos as u64)
}
