use super::{error::result_from_libc, *};

define_struct! {
	pub struct TimeVal {
		pub sec: i64,
		pub micros: i64
	}
}

define_struct! {
	pub struct TimeSpec {
		pub sec: i64,
		pub nanos: i64
	}
}

impl TimeSpec {
	pub fn from_ms(duration: u64) -> Self {
		Self {
			sec: (duration / 1000) as i64,
			nanos: ((duration % 1000) * 1_000_000) as i64
		}
	}
}

define_enum! {
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
}

extern "C" {
	fn clock_gettime(clock: ClockId, spec: &mut TimeSpec) -> i32;
}

pub fn time(clock: ClockId) -> Result<u64> {
	let mut ts = TimeSpec { sec: 0, nanos: 0 };

	result_from_libc(unsafe { clock_gettime(clock, &mut ts) } as isize)?;

	Ok((ts.sec as u64) * 1_000_000_000 + ts.nanos as u64)
}
