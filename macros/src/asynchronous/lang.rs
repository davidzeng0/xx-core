use super::*;

pub fn task_lang_impl(use_lang: TokenStream, item: ItemStruct, attrs: &[Attribute]) -> TokenStream {
	let ident = &item.ident;
	let context = Context::new();
	let context_ident = &context.ident;

	quote! {
		#item

		impl<F: FnOnce(&Context) -> Output, Output> #ident<F, Output> {
			#[inline(always)]
			pub const fn new(func: F) -> Self {
				#use_lang

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
	}
}

pub fn async_closure_impl(use_lang: TokenStream, item: ItemStruct) -> TokenStream {
	let ident = &item.ident;

	quote! {
		#item

		impl<F, const T: usize> #ident<F, T> {
			#[inline(always)]
			pub const fn new(func: F) -> Self {
				#use_lang

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
	}
}
