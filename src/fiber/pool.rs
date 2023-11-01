use std::sync::Mutex;

use crate::{fiber::*, trace};

pub struct Pool {
	pool: Mutex<Vec<Fiber>>,

	/* count does not need to be atomic as it's only modified with the lock */
	count: u64
}

impl Pool {
	pub const fn new() -> Self {
		Self { pool: Mutex::new(Vec::new()), count: 0 }
	}

	pub fn new_fiber(&mut self, start: Start) -> Fiber {
		let mut pool = self.pool.lock().unwrap();

		self.count = self.count.checked_add(1).unwrap();

		match pool.pop() {
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
		let mut pool = self.pool.lock().unwrap();

		self.count = self.count.checked_sub(1).unwrap();

		let ideal = Self::calculate_ideal(self.count);

		if pool.len() < ideal as usize {
			trace!(target: self, "== Preserving worker stack");

			pool.push(fiber);
		}
	}
}
