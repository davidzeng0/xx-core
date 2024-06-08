use super::*;

pub struct CustomError<E> {
	backtrace: Option<Backtrace>,
	error: E
}

impl<E: ErrorImpl> Display for CustomError<E> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		Display::fmt(&self.error, fmt)
	}
}

impl<E: ErrorImpl> Debug for CustomError<E> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		Debug::fmt(&self.error, fmt)
	}
}

impl<E: ErrorImpl> Error for CustomError<E> {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		self.error.source()
	}
}

impl<E: ErrorImpl> ErrorImpl for CustomError<E> {
	fn kind(&self) -> ErrorKind {
		self.error.kind()
	}
}

impl<E> CustomError<E> {
	unsafe fn downcast_ptr<I>(this: MutNonNull<()>, type_id: TypeId) -> Option<MutNonNull<()>>
	where
		I: ErrorBounds
	{
		let this = this.cast::<Self>();

		if type_id != TypeId::of::<I>() {
			return None;
		}

		/* Safety: valid ptr */
		let ptr = unsafe { ptr!(!null &mut this=>error) };

		Some(ptr.cast())
	}

	unsafe fn downcast_owned<I>(
		this: MutNonNull<()>, type_id: TypeId, out: MutPtr<MaybeUninit<()>>
	) -> bool
	where
		I: ErrorBounds
	{
		let this = this.cast::<DynError<CustomError<ManuallyDrop<I>>>>();
		let out = out.cast::<MaybeUninit<I>>();

		if type_id != TypeId::of::<I>() {
			return false;
		}

		/* Safety: get the inner object */
		let error = unsafe { ManuallyDrop::take(&mut ptr!(this=>data.error)) };

		/* Safety: move the error object */
		unsafe { ptr!(out=>write(error)) };

		/* Safety: drop the rest. guaranteed by caller */
		drop(unsafe { Box::from_raw(this.as_mut_ptr()) });

		true
	}

	unsafe fn backtrace(this: MutNonNull<()>) -> Option<&'static Backtrace> {
		let this = this.cast::<Self>();

		/* Safety: valid ptr */
		let bt = unsafe { ptr!(this=>backtrace.as_ref()) };

		/* Safety: guaranteed by caller */
		unsafe { transmute(bt) }
	}

	unsafe fn drop(this: MutNonNull<()>) {
		let this = this.cast::<DynError<Self>>();

		/* Safety: guaranteed by caller */
		drop(unsafe { Box::from_raw(this.as_mut_ptr()) });
	}

	fn into_dyn(self, vtable: &'static ErrorVTable) -> MutNonNull<DynError<()>> {
		MutNonNull::from_box(Box::new(DynError { vtable, data: self })).cast()
	}
}

macro_rules! wrapper_type {
	($name:ident $($bounds:tt)*) => {
		#[repr(transparent)]
		struct $name<E>(pub E);

		impl<E: ErrorBounds> Display for $name<E> {
			fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
				Display::fmt(&self.0, fmt)
			}
		}

		impl<E: ErrorBounds> Debug for $name<E> {
			fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
				Debug::fmt(&self.0, fmt)
			}
		}

		impl<E: ErrorBounds $($bounds)*> ErrorImpl for $name<E> {
			fn kind(&self) -> ErrorKind {
				ErrorKind::Other
			}
		}
	}
}

wrapper_type!(Basic);

impl<E: ErrorBounds> Error for Basic<E> {}

impl<E: ErrorBounds> CustomError<E> {
	pub fn new_basic(error: E) -> MutNonNull<DynError<()>> {
		CustomError::new(Basic(error)).into_dyn(&ErrorVTable {
			kind: |_| ErrorKind::Other,
			meta: CustomError::<Basic<E>>::meta,
			downcast_ptr: Self::downcast_ptr::<E>,
			downcast_owned: Self::downcast_owned::<E>,
			backtrace: Self::backtrace,
			drop: Self::drop
		})
	}
}

wrapper_type!(Std + Error);

impl<E: ErrorBounds + Error> Error for Std<E> {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		self.0.source()
	}
}

impl<E: ErrorBounds + Error> CustomError<E> {
	pub fn new_std(error: E) -> MutNonNull<DynError<()>> {
		CustomError::new(Std(error)).into_dyn(&ErrorVTable {
			kind: |_| ErrorKind::Other,
			meta: CustomError::<Std<E>>::meta,
			downcast_ptr: Self::downcast_ptr::<E>,
			downcast_owned: Self::downcast_owned::<E>,
			backtrace: Self::backtrace,
			drop: Self::drop
		})
	}
}

#[repr(transparent)]
struct Boxed(pub BoxedError);

impl Display for Boxed {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		Display::fmt(&self.0, fmt)
	}
}

impl Debug for Boxed {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		Debug::fmt(&self.0, fmt)
	}
}

impl Error for Boxed {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		self.0.source()
	}
}

impl ErrorImpl for Boxed {
	fn kind(&self) -> ErrorKind {
		ErrorKind::Other
	}
}

impl CustomError<BoxedError> {
	pub fn new_boxed(error: BoxedError) -> MutNonNull<DynError<()>> {
		CustomError::new(Boxed(error)).into_dyn(&ErrorVTable {
			kind: |_| ErrorKind::Other,
			meta: CustomError::<Boxed>::meta,
			downcast_ptr: Self::downcast_ptr::<BoxedError>,
			downcast_owned: Self::downcast_owned::<BoxedError>,
			backtrace: Self::backtrace,
			drop: Self::drop
		})
	}
}

impl<E: ErrorImpl> CustomError<E> {
	unsafe fn kind(this: MutNonNull<()>) -> ErrorKind {
		let this = this.cast::<Self>();

		/* Safety: valid ptr */
		unsafe { ptr!(this=>error.kind()) }
	}

	unsafe fn meta(this: MutNonNull<()>) -> MutNonNull<dyn ErrorImpl> {
		let this = this.cast::<Self>().as_mut_ptr() as *mut dyn ErrorImpl;

		/* Safety: `this` is non-null */
		unsafe { MutNonNull::new_unchecked(this.into()) }
	}

	fn new(error: E) -> Self {
		Self { backtrace: capture_backtrace(), error }
	}

	pub fn new_error_impl(error: E) -> MutNonNull<DynError<()>> {
		Self::new(error).into_dyn(&ErrorVTable {
			kind: Self::kind,
			meta: Self::meta,
			downcast_ptr: Self::downcast_ptr::<E>,
			downcast_owned: Self::downcast_owned::<E>,
			backtrace: Self::backtrace,
			drop: Self::drop
		})
	}
}
