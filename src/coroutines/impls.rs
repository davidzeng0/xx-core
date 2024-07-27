use super::*;
use crate::macros::{macro_each, paste};

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
	(([$($self:tt)*] [$async_kind:ident] $([$call:ident, $kind:tt])?)) => {
		paste! {
			impl<F, T, Args, Output> [< AsyncFn $($kind)? >] <Args> for F
			where
				F: [< Fn $($kind)? >] (Args) -> T,
				T: for<'a> Task<Output<'a> = Output>
			{
				type Output = Output;

				#[cfg(not(any(doc, feature = "xx-doc")))]
				#[asynchronous($async_kind)]
				async fn [< call $($call)? >](
					$($self)* self, args: Args
				) -> Self::Output {
					self(args).await
				}

				#[cfg(any(doc, feature = "xx-doc"))]
				async fn [< call $($call)? >](
					$($self)* self, args: Args
				) -> Self::Output {}
			}
		}
	};
}

macro_each!(async_fn, ([] [traitext] [_once, Once]), ([&mut] [traitfn] [_mut, Mut]), ([&] [traitfn]));

macro_rules! map_fn {
	(
		($($call:ident, $kind:tt)?)
	) => {
		paste! {
			#[asynchronous(sync)]
			pub trait [< AsyncFn $($kind)? Map >] <Args>: [< AsyncFn $($kind)? >] <Args> + Sized {
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
				[< AsyncFn $($kind)? Map >] <Args> for F {}
		}
	};
}

macro_each!(map_fn, (_once, Once), (_mut, Mut), ());
