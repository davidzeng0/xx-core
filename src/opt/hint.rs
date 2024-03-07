#[inline(always)]
#[cold]
fn cold() {}

#[inline(always)]
pub fn likely(cond: bool) -> bool {
	if !cond {
		cold();
	}

	cond
}

#[inline(always)]
pub fn unlikely(cond: bool) -> bool {
	if cond {
		cold();
	}

	cond
}

#[inline(always)]
pub unsafe fn unreachable_unchecked() {
	#[cfg(debug_assertions)]
	assert!(false);
	#[cfg(not(debug_assertions))]
	std::hint::unreachable_unchecked();
}
