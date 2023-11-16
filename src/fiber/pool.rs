use std::sync::Mutex;

use crate::{fiber::*, trace};

struct Data {
	pool: Vec<Fiber>,
	active: u64
}

impl Data {
	const fn new() -> Self {
		Self { pool: Vec::new(), active: 0 }
	}
}

pub struct Pool {
	data: Mutex<Data>
}

impl Pool {
	pub const fn new() -> Self {
		Self { data: Mutex::new(Data::new()) }
	}

	pub fn new_fiber(&mut self, start: Start) -> Fiber {
		let mut data = self.data.lock().unwrap();

		data.active = data.active.checked_add(1).unwrap();

		match data.pool.pop() {
			Some(mut fiber) => {
				trace!(target: self, "== Reusing stack for worker");

				unsafe { fiber.set_start(start) }

				return fiber;
			}

			None => {
				trace!(target: self, "== Creating stack for worker");

				Fiber::new_with_start(start)
			}
		}
	}

	fn calculate_ideal(count: u64) -> u64 {
		const RATIO: u64 = 20;

		count * RATIO / 100 + 16
	}

	pub fn exit_fiber(&mut self, fiber: Fiber) {
		let mut data = self.data.lock().unwrap();

		data.active = data.active.checked_sub(1).unwrap();

		let ideal = Self::calculate_ideal(data.active);

		if data.pool.len() < ideal as usize {
			trace!(target: self, "== Preserving worker stack");

			data.pool.push(fiber);
		} else {
			trace!(target: self, "== Dropping worker stack");
		}
	}
}
