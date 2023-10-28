use std::sync::Mutex;

use crate::{fiber::*, task::*, trace};

pub struct Pool {
	pool: Mutex<Vec<Fiber>>,
	count: u64
}

impl Global for Pool {}

impl Pool {
	pub const fn new() -> Self {
		Self { pool: Mutex::new(Vec::new()), count: 0 }
	}

	pub fn new_fiber(&mut self, start: Start) -> Fiber {
		let mut pool = self.pool.lock().unwrap();

		self.count += 1;

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

		self.count -= 1;

		let ideal = Self::calculate_ideal(self.count);

		if pool.len() < ideal as usize {
			trace!(target: self, "== Preserving worker stack");

			pool.push(fiber);
		}
	}
}
