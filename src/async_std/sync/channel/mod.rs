use super::*;
use crate::sync::{Backoff, CachePadded};

mod error;
mod mp;
use self::error::*;
use self::mp::*;

pub mod mpmc;
pub mod mpsc;
pub mod oneshot;

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
