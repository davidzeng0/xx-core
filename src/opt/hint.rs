#[inline]
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
