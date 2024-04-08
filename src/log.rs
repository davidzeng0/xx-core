use std::{
	any::type_name,
	backtrace::*,
	fmt::{self, Arguments},
	io::*,
	panic::*,
	str::from_utf8,
	sync::*
};

use ctor::ctor;
use lazy_static::lazy_static;
use log::{set_boxed_logger, Log, Metadata, Record};
pub use log::{set_max_level, Level, LevelFilter};

use crate::{macros::panic_nounwind, pointer::*};

pub mod internal {
	pub use log::{log, log_enabled};

	use super::*;

	#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
	fn get_struct_name<T>() -> &'static str
	where
		T: ?Sized
	{
		let full_name = type_name::<T>();

		let mut start = None;
		let mut angle_brackets = 0;
		let mut segment = full_name;

		for (index, ch) in full_name.char_indices() {
			start.get_or_insert(index);

			if ch == '<' {
				if angle_brackets == 0 {
					segment = &full_name[start.unwrap()..index];
				}

				angle_brackets += 1;
			}

			if ch == '>' {
				angle_brackets -= 1;

				if angle_brackets == 0 {
					start = None;
				}
			}
		}

		segment.split("::").last().unwrap()
	}

	fn get_struct_addr_low<T, const MUT: bool>(val: Pointer<T, MUT>) -> usize
	where
		T: ?Sized
	{
		val.int_addr() & u32::MAX as usize
	}

	#[inline(never)]
	pub fn log_target<T, const MUT: bool>(
		level: Level, target: Pointer<T, MUT>, args: Arguments<'_>
	) where
		T: ?Sized
	{
		let mut fmt_buf = Cursor::new([0u8; 64]);
		let _ = fmt_buf.write_fmt(format_args!(
			"@ {:0>8x} {: >13}",
			get_struct_addr_low(target),
			get_struct_name::<T>()
		));

		#[allow(clippy::cast_possible_truncation)]
		let pos = fmt_buf.position() as usize;

		log!(
			target: from_utf8(&fmt_buf.get_ref()[0..pos]).unwrap_or("<error>"),
			level,
			"{}",
			args
		);
	}

	pub(super) fn print_fatal(thread_name: &str, fmt: Arguments<'_>) {
		log!(target: thread_name, Level::Error, "{}", fmt);
	}

	pub(super) fn print_backtrace(thread_name: &str) {
		let backtrace = Backtrace::capture();

		log!(
			target: thread_name,
			Level::Error,
			"{:?}",
			backtrace
		);
	}
}

struct Logger;

lazy_static! {
	static ref STDERR: Mutex<BufWriter<Stderr>> =
		Mutex::new(BufWriter::with_capacity(1024, stderr()));
}

fn get_stderr() -> MutexGuard<'static, BufWriter<Stderr>> {
	#[allow(clippy::unwrap_used)]
	STDERR.lock().unwrap()
}

macro_rules! ansi_color {
	(bold) => {
		format_args!("{}", "\x1b[1m")
	};

	($color:expr) => {
		format_args!("\x1b[1;48;5;{}m", $color)
	};

	() => {
		format_args!("{}", "\x1b[0m")
	};
}

struct Adapter<'a> {
	output: MutexGuard<'a, BufWriter<Stderr>>,
	record: &'a Record<'a>,
	wrote_prefix: bool
}

impl Adapter<'_> {
	fn write_prefix_with_color(&mut self, color: Arguments<'_>) -> Result<()> {
		self.output.write_fmt(format_args!(
			"{}| {: >24} |{} ",
			color,
			self.record.target(),
			ansi_color!()
		))
	}

	fn write_prefix(&mut self) -> Result<()> {
		if self.wrote_prefix {
			return Ok(());
		}

		let result = match self.record.level() {
			Level::Error => self.write_prefix_with_color(ansi_color!(1)),
			Level::Warn => self.write_prefix_with_color(ansi_color!(11)),
			Level::Info => self.write_prefix_with_color(ansi_color!(10)),
			Level::Debug => self.write_prefix_with_color(ansi_color!(14)),
			Level::Trace => self.write_prefix_with_color(ansi_color!(bold))
		};

		self.wrote_prefix = true;

		result
	}
}

impl fmt::Write for Adapter<'_> {
	fn write_str(&mut self, data: &str) -> fmt::Result {
		if !data.contains('\n') {
			self.write_prefix().map_err(|_| fmt::Error)?;
			self.output
				.write_all(data.as_bytes())
				.map_err(|_| fmt::Error)?;

			return Ok(());
		}

		for line in data.split_inclusive('\n') {
			self.write_prefix().map_err(|_| fmt::Error)?;
			self.output
				.write_all(line.as_bytes())
				.map_err(|_| fmt::Error)?;
			if line.ends_with('\n') {
				self.wrote_prefix = false;
			}
		}

		Ok(())
	}
}

impl Log for Logger {
	fn enabled(&self, _: &Metadata<'_>) -> bool {
		true
	}

	fn log(&self, record: &Record<'_>) {
		if !self.enabled(record.metadata()) {
			return;
		}

		let mut adapter = Adapter { output: get_stderr(), record, wrote_prefix: false };

		let _ = fmt::Write::write_fmt(&mut adapter, *record.args());

		if adapter.wrote_prefix {
			let _ = adapter.output.write_all(&[b'\n']);
		}

		let _ = adapter.output.flush();
	}

	fn flush(&self) {
		let _ = get_stderr().flush();
	}
}

macro_rules! get_thread_name {
	($var:ident) => {
		let thread = ::std::thread::current();
		let $var = thread.name().unwrap_or("<unnamed>");
	};
}

pub fn print_backtrace() {
	get_thread_name!(thread_name);

	internal::print_backtrace(thread_name);
}

pub fn print_fatal(fmt: Arguments<'_>) {
	get_thread_name!(thread_name);

	internal::print_fatal(thread_name, fmt);
}

pub fn print_panic(location: Option<&Location<'_>>, fmt: Arguments<'_>) {
	get_thread_name!(thread_name);

	let location = if let Some(location) = location {
		location
	} else {
		Location::caller()
	};

	internal::print_fatal(
		thread_name,
		format_args!("Panic occurred at {}:\n>> {}", location, fmt)
	);

	let backtrace = Backtrace::capture();

	if backtrace.status() == BacktraceStatus::Captured {
		internal::print_fatal(thread_name, format_args!("\nBack trace:\n{}", backtrace));
	} else {
		internal::print_fatal(
			thread_name,
			format_args!(
				"note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace"
			)
		);
	}
}

fn panic_hook(info: &PanicInfo<'_>) {
	let msg = match info.payload().downcast_ref::<&'static str>() {
		Some(s) => *s,
		None => match info.payload().downcast_ref::<String>() {
			Some(s) => &s[..],
			None => "Box<dyn Any>"
		}
	};

	print_panic(info.location(), format_args!("{}", msg));
}

#[cfg(feature = "log")]
#[ctor]
fn init() {
	if set_boxed_logger(Box::new(Logger)).is_err() {
		panic_nounwind!("Failed to initialize logger");
	}

	#[cfg(feature = "panic-log")]
	set_hook(Box::new(panic_hook));

	set_max_level(LevelFilter::Info);
}

#[macro_export]
macro_rules! log {
	($level: expr, target: $target: expr, $($arg: tt)+) => {
		if $crate::opt::hint::unlikely($crate::log::internal::log_enabled!($level)) {
			$crate::log::internal::log_target(
				$level,
				$crate::macros::ptr!($target),
				format_args!($($arg)+)
			);
		}
	};

	($level: expr, $($arg: tt)+) => {
		$crate::log::internal::log!($level, $($arg)+)
	};
}

#[macro_export]
macro_rules! error {
	($($arg: tt)+) => {
		$crate::log!($crate::log::Level::Error, $($arg)+)
	}
}

#[macro_export]
macro_rules! warn {
	($($arg: tt)+) => {
		$crate::log!($crate::log::Level::Warn, $($arg)+)
	}
}

#[macro_export]
macro_rules! info {
	($($arg: tt)+) => {
		$crate::log!($crate::log::Level::Info, $($arg)+)
	}
}

#[macro_export]
macro_rules! debug {
	($($arg: tt)+) => {
		$crate::log!($crate::log::Level::Debug, $($arg)+)
	}
}

#[macro_export]
macro_rules! trace {
	($($arg: tt)+) => {
		$crate::log!($crate::log::Level::Trace, $($arg)+)
	}
}
