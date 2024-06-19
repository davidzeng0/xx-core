use std::{
	mem::MaybeUninit,
	result,
	sync::{atomic::*, Arc}
};

use super::*;

mod mp;
use mp::*;

pub mod mpmc;
pub mod mpsc;
pub mod oneshot;

#[errors]
pub enum RecvError {
	#[error("Channel empty")]
	#[kind = ErrorKind::WouldBlock]
	Empty,

	#[error("Channel closed")]
	Closed
}

impl RecvError {
	#[must_use]
	const fn new(closed: bool) -> Self {
		if closed {
			Self::Closed
		} else {
			Self::Empty
		}
	}
}

#[errors]
pub enum SendError<T> {
	#[error("Channel full")]
	#[kind = ErrorKind::WouldBlock]
	Full(T),

	#[error("Channel closed")]
	Closed(T)
}

impl<T> SendError<T> {
	#[must_use]
	const fn new(value: T, closed: bool) -> Self {
		if closed {
			Self::Closed(value)
		} else {
			Self::Full(value)
		}
	}
}

type RecvResult<T> = result::Result<T, RecvError>;
type SendResult<T> = result::Result<(), SendError<T>>;
