use std::{
	any::type_name,
	io::{stderr, Write}
};

use ctor::ctor;
use log::{set_boxed_logger, set_max_level, Level, LevelFilter, Log, Metadata, Record};

struct Logger;

macro_rules! ansi_color {
	($color: expr) => {
		format!("\x1b[38;5;{}m", $color)
	};

	() => {
		"\x1b[0m"
	};
}

impl Log for Logger {
	fn enabled(&self, _: &Metadata) -> bool {
		true
	}

	fn log(&self, record: &Record) {
		if !self.enabled(record.metadata()) {
			return;
		}

		let color = match record.level() {
			Level::Error => ansi_color!(1),
			Level::Warn => ansi_color!(3),
			Level::Info => ansi_color!(122),
			Level::Debug => ansi_color!(14),
			Level::Trace => ansi_color!(224)
		};

		let content = format!("{}", record.args());
		let lines = content.lines();

		let target = record.target();

		for line in lines {
			let line = format!("{}[ {: >32} ] {}{}\n", color, target, ansi_color!(), line);

			let _ = stderr().write_all(line.as_bytes());
		}
	}

	fn flush(&self) {}
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
		"@ {:0>16x} : {: >11}",
		get_struct_addr(val),
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
