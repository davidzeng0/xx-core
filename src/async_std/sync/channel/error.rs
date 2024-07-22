#![allow(clippy::module_name_repetitions)]

use super::*;

#[errors]
pub enum RecvError {
	#[display("Channel empty")]
	#[kind = ErrorKind::WouldBlock]
	Empty,

	#[display("Channel closed")]
	Closed
}

#[errors(?Debug + ?Display)]
pub enum SendError<T> {
	#[fmt("Channel full")]
	#[kind = ErrorKind::WouldBlock]
	Full(T),

	#[fmt("Channel closed")]
	Closed(T)
}
