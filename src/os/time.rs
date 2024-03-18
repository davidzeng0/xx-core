use std::time::Duration;

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

#[allow(clippy::unwrap_used)]
impl TimeSpec {
	/// # Panics
	/// if the input cannot fit in the resulting Duration
	#[must_use]
	pub fn from_ms(duration: u64) -> Self {
		let subsec_millis: i64 = (duration % 1000).try_into().unwrap();

		Self {
			sec: (duration / 1000).try_into().unwrap(),
			nanos: subsec_millis.checked_mul(1_000_000).unwrap()
		}
	}

	/// # Panics
	/// if the input cannot fit in the resulting Duration
	#[must_use]
	pub fn from_nanos(duration: u64) -> Self {
		Self {
			sec: (duration / 1_000_000_000).try_into().unwrap(),
			nanos: (duration % 1_000_000_000).try_into().unwrap()
		}
	}

	/// # Panics
	/// if the input cannot fit in the resulting Duration
	#[must_use]
	pub fn from_duration(duration: Duration) -> Self {
		Self {
			sec: duration.as_secs().try_into().unwrap(),
			nanos: duration.subsec_nanos() as i64
		}
	}

	/// # Panics
	/// if the input cannot fit in the resulting Duration
	#[must_use]
	pub fn from_ms_i32(duration: i32) -> Self {
		Self::from_ms(duration.try_into().unwrap())
	}
}

define_enum! {
	#[repr(i32)]
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

	/* Safety: FFI call */
	result_from_libc(unsafe { clock_gettime(clock, &mut ts) } as isize)?;

	let overflow = |_| Core::Overflow.as_err();
	let sec: u64 = ts.sec.try_into().map_err(overflow)?;
	let nanos: u64 = ts.nanos.try_into().map_err(overflow)?;

	sec.checked_mul(1_000_000_000)
		.and_then(|time| time.checked_add(nanos))
		.ok_or_else(|| Core::Overflow.as_err())
}
