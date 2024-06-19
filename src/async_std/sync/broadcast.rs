#![allow(warnings)]

use std::{mem::MaybeUninit, sync::Arc};

use super::*;

struct Slot<T> {
	data: UnsafeCell<MaybeUninit<T>>
}

struct Channel<T> {
	slots: Box<[Slot<T>]>
}

pub struct Receiver<T> {
	channel: Arc<Channel<T>>
}

pub struct Sender<T> {
	channel: Arc<Channel<T>>
}

pub fn channel<T: Clone>(size: usize) -> (Sender<T>, Receiver<T>) {
	todo!();
}