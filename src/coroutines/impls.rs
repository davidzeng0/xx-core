use paste::paste;

use super::*;
use crate::macros::macro_each;

#[asynchronous(traitext)]
pub trait TaskExt: Task + Sized {
	async fn map<F, V, O>(self, map: F) -> O
	where
		F: AsyncFnOnce(V) -> O,
		Self: for<'ctx> Task<Output<'ctx> = V>
	{
		map.call_once(self.await).await
	}

	async fn map_sync<F, V, O>(self, map: F) -> O
	where
		F: FnOnce(V) -> O,
		Self: for<'ctx> Task<Output<'ctx> = V>
	{
		map(self.await)
	}
}

impl<T: Task> TaskExt for T {}

macro_rules! async_fn {
	(([$($self:tt)*] $(, $call:ident, $kind:tt)?)) => {
		paste! {
			impl<F, T, Args, Output> [< AsyncFn $($kind)? >] <Args> for F
			where
				F: [< Fn $($kind)? >] (Args) -> T,
				T: for<'a> Task<Output<'a> = Output>
			{
				type Output = Output;

				fn [< call $($call)? >](
					$($self)* self, args: Args
				) -> impl for<'ctx> Task<Output<'ctx> = Self::Output> {
					self(args)
				}
			}
		}
	};
}

macro_each!(async_fn, ([], _once, Once), ([&mut], _mut, Mut), ([&]));

macro_rules! map_fn {
	(
		($($call:ident, $kind:tt)?)
	) => {
		paste! {
			#[asynchronous(sync)]
			pub trait [< AsyncFn $($kind)? Ext >] <Args>: [< AsyncFn $($kind)? >] <Args> + Sized {
				#[allow(unused_mut)]
				fn [< map $($call)? >] <F>(
					mut self, mut map: F
				) -> impl [< AsyncFn $($kind)? >] <Args, Output = F::Output>
				where
					F: [< AsyncFn $($kind)? >] <Self::Output>
				{
					move |args: Args| async move {
						map. [< call $($call)? >] (
							self. [< call $($call)? >] (args).await
						).await
					}
				}

				#[allow(unused_mut)]
				fn [< map $($call)? _sync >] <F, Output>(
					mut self, mut map: F
				) -> impl [< AsyncFn $($kind)? >] <Args, Output = Output>
				where
					F: [< Fn $($kind)? >](Self::Output) -> Output
				{
					move |args: Args| async move {
						map(self. [< call $($call)? >] (args).await)
					}
				}
			}

			impl<F: [< AsyncFn $($kind)? >] <Args>, Args>
				[< AsyncFn $($kind)? Ext >] <Args> for F {}
		}
	};
}

macro_each!(map_fn, (_once, Once), (_mut, Mut), ());
