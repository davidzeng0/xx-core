#[cfg(test)]
mod test {
	use crate::{
		fiber::{Fiber, Start},
		pointer::{MutPtr, Ptr}
	};

	fn start(arg: Ptr<()>) {
		let mut data = arg.cast::<(Fiber, Fiber, i32)>().cast_mut();
		let mut val = 0;

		loop {
			data.2 += val;
			val += 1;

			unsafe {
				data.as_mut().1.switch(&mut data.0);
			}
		}
	}

	#[test]
	fn test_fibers() {
		let mut data = (Fiber::main(), Fiber::new(), 0i32);
		let mut data = MutPtr::from(&mut data);

		unsafe {
			data.as_mut()
				.1
				.set_start(Start::new(start, data.as_unit().into()));
		}

		let mut val = 0;

		for i in 0..10 {
			unsafe {
				data.as_mut().0.switch(&mut data.1);
			}

			val += i;

			assert_eq!(data.2, val);
		}

		data.2 = 0;

		unsafe {
			data.as_mut().0.switch(&mut data.1);
		}

		assert_eq!(data.2, 10);
	}
}
