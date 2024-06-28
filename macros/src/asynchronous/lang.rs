use super::*;

pub fn task_lang_impl(use_lang: TokenStream, item: ItemStruct, attrs: &[Attribute]) -> TokenStream {
	let ident = &item.ident;
	let context = Context::new();
	let context_ident = &context.ident;

	quote! {
		#item

		const _: () = {
			use ::std::ops::FnOnce;
			use ::std::marker::PhantomData;
			use ::xx_core::coroutines::Context;

			#use_lang

			impl<F: FnOnce(&Context) -> Output, Output> #ident<F, Output> {
				#[inline(always)]
				pub const fn new(func: F) -> Self {
					Self(func, PhantomData)
				}
			}

			unsafe impl<F: FnOnce(&Context) -> Output, Output> Task for #ident<F, Output> {
				type Output<'ctx> = Output;

				#(#attrs)*
				fn run(self, #context) -> Output {
					self.0(#context_ident)
				}
			}
		};
	}
}

pub fn async_closure_impl(use_lang: TokenStream, item: ItemStruct) -> TokenStream {
	let ident = &item.ident;

	quote! {
		#item

		const _: () = {
			use ::std::ops::FnOnce;
			use ::xx_core::coroutines::{asynchronous, get_context, Context};

			#use_lang

			impl<F, const T: usize> #ident<F, T> {
				#[inline(always)]
				pub const fn new(func: F) -> Self {
					Self(func)
				}
			}

			impl<F: FnOnce(Args, &Context) -> Output, Args, Output> AsyncFnOnce<Args> for #ident<F, 0> {
				type Output = Output;

				#[asynchronous(traitext)]
				#[inline(always)]
				async fn call_once(self, args: Args) -> Output {
					self.0(args, unsafe { get_context().await })
				}
			}

			impl<F: FnMut(Args, &Context) -> Output, Args, Output> AsyncFnMut<Args> for #ident<F, 1> {
				type Output = Output;

				#[asynchronous(traitext)]
				#[inline(always)]
				async fn call_mut(&mut self, args: Args) -> Output {
					self.0(args, unsafe { get_context().await })
				}
			}

			impl<F: Fn(Args, &Context) -> Output, Args, Output> AsyncFn<Args> for #ident<F, 2> {
				type Output = Output;

				#[asynchronous(traitext)]
				#[inline(always)]
				async fn call(&self, args: Args) -> Output {
					self.0(args, unsafe { get_context().await })
				}
			}
		};
	}
}
