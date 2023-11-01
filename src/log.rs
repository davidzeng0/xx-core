use std::{
	any::type_name,
	fmt,
	io::{stderr, BufWriter, Cursor, Result, Stderr, Write},
	str::from_utf8_unchecked,
	sync::{Mutex, MutexGuard}
};

use ctor::ctor;
use lazy_static::lazy_static;
use log::{set_boxed_logger, set_max_level, Level, LevelFilter, Log, Metadata, Record};

struct Logger;

lazy_static! {
	static ref STDERR: Mutex<BufWriter<Stderr>> =
		Mutex::new(BufWriter::with_capacity(1024, stderr()));
}

fn get_stderr() -> MutexGuard<'static, BufWriter<Stderr>> {
	STDERR.lock().unwrap()
}

macro_rules! ansi_color {
	(bold) => {
		format_args!("{}", "\x1b[1m")
	};

	($color: expr) => {
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

impl<'a> Adapter<'a> {
	fn write_prefix_with_color(&mut self, color: fmt::Arguments<'_>) -> Result<()> {
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

impl<'a> fmt::Write for Adapter<'a> {
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
	fn enabled(&self, _: &Metadata) -> bool {
		true
	}

	fn log(&self, record: &Record) {
		if !self.enabled(record.metadata()) {
			return;
		}

		let mut adapter = Adapter { output: get_stderr(), record, wrote_prefix: false };

		let _ = fmt::Write::write_fmt(&mut adapter, record.args().clone());
		let _ = adapter.output.write_all(&[b'\n']);
		let _ = adapter.output.flush();
	}

	fn flush(&self) {
		let _ = get_stderr().flush();
	}
}

#[ctor]
fn init() {
	set_boxed_logger(Box::new(Logger)).expect("Failed to initialize logger");
	set_max_level(LevelFilter::Info)
}

fn get_struct_name<T>(_: &T) -> &str {
	type_name::<T>().split("::").last().unwrap()
}

fn get_struct_addr<T>(val: &T) -> usize {
	val as *const _ as usize
}

fn get_struct_addr_low<T>(val: &T) -> usize {
	get_struct_addr(val) & 0xffffffff
}

#[inline(never)]
pub fn log_target<T>(level: Level, target: &T, args: fmt::Arguments<'_>) {
	let mut fmt_buf = Cursor::new([0u8; 64]);

	fmt_buf
		.write_fmt(format_args!(
			"@ {:0>8x} {: >13}",
			get_struct_addr_low(target),
			get_struct_name(target)
		))
		.expect("Log struct name too long");

	let pos = fmt_buf.position() as usize;

	log::log!(
		target: unsafe { from_utf8_unchecked(&fmt_buf.get_ref()[0..pos]) },
		level,
		"{}",
		args
	);
}

#[macro_export]
macro_rules! log {
	($level: expr, target: $target: expr, $($arg: tt)+) => {
		if $crate::opt::hint::unlikely(::log::log_enabled!($level)) {
			$crate::log::log_target($level, $target, format_args!($($arg)+));
		}
	};

	($level: expr, $($arg: tt)+) => {
		::log::log!($level, $($arg)+)
	};
}

#[macro_export]
macro_rules! error {
	($($arg: tt)+) => {
		$crate::log!(::log::Level::Error, $($arg)+)
	}
}

#[macro_export]
macro_rules! warn {
	($($arg: tt)+) => {
		$crate::log!(::log::Level::Warn, $($arg)+)
	}
}

#[macro_export]
macro_rules! info {
	($($arg: tt)+) => {
		$crate::log!(::log::Level::Info, $($arg)+)
	}
}

#[macro_export]
macro_rules! debug {
	($($arg: tt)+) => {
		$crate::log!(::log::Level::Debug, $($arg)+)
	}
}

#[macro_export]
macro_rules! trace {
	($($arg: tt)+) => {
		$crate::log!(::log::Level::Trace, $($arg)+)
	}
}
