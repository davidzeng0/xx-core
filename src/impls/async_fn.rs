use super::*;

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
