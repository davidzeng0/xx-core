use ctor::ctor;
use log::{set_boxed_logger, set_max_level, Level, LevelFilter, Log, Metadata, Record};
use std::any::type_name;

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
			Level::Trace => ansi_color!(244)
		};

		let target = record.target();

		eprintln!("{}[{}] {}{}", color, target, ansi_color!(), record.args());
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

fn get_struct_addr<T>(val: &T) -> String {
	format!("{:p}", val as *const T)
}

pub fn format_target<T>(val: &T) -> String {
	format!("{} @ {}", get_struct_name(val), get_struct_addr(val))
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
