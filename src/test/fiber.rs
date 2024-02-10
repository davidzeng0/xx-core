#[cfg(test)]
mod test {
	use crate::{
		fiber::{Fiber, Start},
		pointer::{MutPtr, Ptr}
	};

	unsafe fn start(arg: Ptr<()>) {
		let mut val = 0;
		let data = arg.cast::<(Fiber, Fiber, i32)>().cast_mut();

		loop {
			data.as_mut().2 += val;
			val += 1;
			data.as_mut().1.switch(&mut data.as_mut().0);
		}
	}

	#[test]
	fn test_fibers() {
		unsafe {
			let mut data = (Fiber::main(), Fiber::new(), 0i32);
			let data = MutPtr::from(&mut data);

			data.as_mut()
				.1
				.set_start(Start::new(start, data.as_unit().into()));

			let mut val = 0;

			for i in 0..10 {
				data.as_mut().0.switch(&mut data.as_mut().1);
				val += i;

				assert_eq!(data.as_ref().2, val);
			}

			data.as_mut().2 = 0;
			data.as_mut().0.switch(&mut data.as_mut().1);

			assert_eq!(data.as_mut().2, 10);
		}
	}
}
