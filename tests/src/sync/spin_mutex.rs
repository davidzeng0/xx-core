use std::{
	panic::catch_unwind,
	sync::{Arc, TryLockError},
	thread
};

use xx_core::sync::SpinMutex;

#[test]
fn test_poison() {
	let mut mutex = SpinMutex::new(5);

	*mutex.lock().unwrap() = 7;

	assert!(*mutex.lock().unwrap() == 7);

	catch_unwind(|| {
		*mutex.lock().unwrap() = 1;

		panic!()
	});

	assert!(*mutex.lock().unwrap() == 1);

	catch_unwind(|| {
		let mut lock = mutex.lock().unwrap();

		*lock = 2;

		panic!()
	});

	assert!(*mutex.lock().unwrap_err().into_inner() == 2);

	mutex.clear_poison();

	let lock = mutex.lock().unwrap();

	match mutex.try_lock().unwrap_err() {
		TryLockError::WouldBlock => (),
		_ => assert!(false)
	}

	drop(lock);

	*mutex.get_mut().unwrap() = 22;

	assert!(*mutex.lock().unwrap() == 22);
}

#[test]
fn test_lock() {
	let mutex = Arc::new(SpinMutex::new(0));
	let mut handles = Vec::new();

	for _ in 0..4 {
		let mtx = mutex.clone();

		handles.push(thread::spawn(move || {
			for _ in 0..1_000 {
				let mut lock = mtx.lock().unwrap();

				*lock += 1;

				drop(lock);
			}
		}));
	}

	for handle in handles {
		handle.join();
	}

	assert_eq!(*Arc::into_inner(mutex).unwrap().get_mut().unwrap(), 4_000);
}
