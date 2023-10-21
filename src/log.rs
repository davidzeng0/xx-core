use std::{
	any::type_name,
	fmt,
	io::{stderr, BufWriter, Result, Stderr, Write},
	ops::{Deref, DerefMut},
	sync::{Mutex, MutexGuard}
};

use ctor::ctor;
use log::{set_boxed_logger, set_max_level, Level, LevelFilter, Log, Metadata, Record};

struct Logger;

struct Output {
	data: Option<BufWriter<Stderr>>
}

impl Deref for Output {
	type Target = BufWriter<Stderr>;

	fn deref(&self) -> &Self::Target {
		self.data.as_ref().unwrap()
	}
}

impl DerefMut for Output {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.data
			.get_or_insert_with(|| BufWriter::with_capacity(1024, stderr()))
	}
}

static mut STDERR: Mutex<Output> = Mutex::new(Output { data: None });

fn get_stderr() -> MutexGuard<'static, Output> {
	unsafe { &STDERR }.lock().unwrap()
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
	output: MutexGuard<'a, Output>,
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
			self.wrote_prefix = false;
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

pub fn format_target<T>(val: &T) -> String {
	format!(
		"@ {:0>8x} {: >13}",
		get_struct_addr(val) & 0xffffffff,
		get_struct_name(val)
	)
}

#[macro_export]
macro_rules! error {
    (target: $target: expr, $($arg: tt)+) => {
        log::error!(
            target: &$crate::log::format_target($target) as &str,
            $($arg)+
        )
    };

    ($($arg: tt)+) => {
        log::error!($($arg)+)
    };
}

#[macro_export]
macro_rules! warn {
    (target: $target: expr, $($arg: tt)+) => {
        log::warn!(
            target: &$crate::log::format_target($target) as &str,
            $($arg)+
        )
    };

    ($($arg: tt)+) => {
        log::warn!($($arg)+)
    };
}

#[macro_export]
macro_rules! info {
    (target: $target: expr, $($arg: tt)+) => {
        log::info!(
            target: &$crate::log::format_target($target) as &str,
            $($arg)+
        )
    };

    ($($arg: tt)+) => {
        log::info!($($arg)+)
    };
}

#[macro_export]
macro_rules! debug {
    (target: $target: expr, $($arg: tt)+) => {
        log::debug!(
            target: &$crate::log::format_target($target) as &str,
            $($arg)+
        )
    };

    ($($arg: tt)+) => {
        log::debug!($($arg)+)
    };
}

#[macro_export]
macro_rules! trace {
    (target: $target: expr, $($arg: tt)+) => {
        log::trace!(
            target: &$crate::log::format_target($target) as &str,
            $($arg)+
        )
    };

    ($($arg: tt)+) => {
        log::trace!($($arg)+)
    };
}
