use paste::paste;

pub trait UIntExtensions {
	type Signed;

	fn overflowing_signed_difference(self, rhs: Self) -> (Self::Signed, bool);
	fn saturating_signed_difference(self, rhs: Self) -> Self::Signed;
}

macro_rules! uint_impl {
	($type: ty, $signed: ty) => {
		impl UIntExtensions for $type {
			type Signed = $signed;

			fn overflowing_signed_difference(self, rhs: Self) -> ($signed, bool) {
				let res = self.wrapping_sub(rhs) as $signed;
				let overflow = (self >= rhs) == (res < 0);

				(res, overflow)
			}

			fn saturating_signed_difference(self, rhs: Self) -> $signed {
				let (res, overflow) = self.overflowing_signed_difference(rhs);

				if !overflow {
					res
				} else if res < 0 {
					<$signed>::MAX
				} else {
					<$signed>::MIN
				}
			}
		}
	};
}

uint_impl!(u8, i8);
uint_impl!(u16, i16);
uint_impl!(u32, i32);
uint_impl!(u64, i64);
uint_impl!(u128, i128);
uint_impl!(usize, isize);

macro_rules! async_fns_type {
	([$($type: ident)?][$($indexes: literal)*] $args: literal $($remaining: literal)*) => {
		paste! {
			pub trait [<AsyncFn $($type)? $args>]<$([<Arg $indexes>]),*>:
				[<Fn $($type)?>]($([<Arg $indexes>]),*) ->
				<Self as [<AsyncFn $($type)? $args>]<$([<Arg $indexes>]),*>>::Future
			{
				type Future: crate::coroutines::Task<Output = <Self as [<AsyncFn $($type)? $args>]<$([<Arg $indexes>]),*>>::Output>;
				type Output;
			}

			impl<'c, 'a, 'f, T, F, $([<Arg $indexes>]),*> [<AsyncFn $($type)? $args>]<$([<Arg $indexes>]),*> for T
			where
				'c: 'f,
				'a: 'f,
				T: [<Fn $($type)?>]($([<Arg $indexes>]),*) -> F + 'c,
				$([<Arg $indexes>]: 'a,)*
				F: crate::coroutines::Task + 'f,
			{
				type Future = F;
				type Output = F::Output;
			}
		}

		async_fns_type! {
			[$($type)?][$($indexes)* $args] $($remaining)*
		}
	};

	([$($type: ident)?][$($indexes: literal)*]) => {};
}

macro_rules! async_fns {
	($($args: literal)*) => {
		async_fns_type! {
			[Once][] $($args)+
		}

		async_fns_type! {
			[Mut][] $($args)+
		}

		async_fns_type! {
			[][] $($args)+
		}
	};
}

/* https://docs.rs/async_fn_traits */
async_fns!(0 1 2 3 4 5 6 7 8 9 10 11 12);

pub trait Captures<'__> {}

impl<T: ?Sized> Captures<'_> for T {}
