use std::mem::MaybeUninit;
use std::result;
use std::sync::atomic::*;
use std::sync::Arc;

use super::*;
use crate::sync::{Backoff, CachePadded};

mod mp;
use mp::*;

pub mod mpmc;
pub mod mpsc;
pub mod oneshot;

mod error {
	#![allow(clippy::module_name_repetitions)]

	use super::*;

	#[errors]
	pub enum RecvError {
		#[error("Channel empty")]
		#[kind = ErrorKind::WouldBlock]
		Empty,

		#[error("Channel closed")]
		Closed
	}

	#[errors]
	pub enum SendError<T> {
		#[error("Channel full")]
		#[kind = ErrorKind::WouldBlock]
		Full(T),

		#[error("Channel closed")]
		Closed(T)
	}
}

use error::*;

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
