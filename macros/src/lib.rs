use proc_macro::TokenStream;

mod async_trait;
mod closure;
mod wrap_function;

#[proc_macro_attribute]
pub fn sync_task(attr: TokenStream, item: TokenStream) -> TokenStream {
	closure::sync_task(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn async_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
	closure::async_fn(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn async_trait(attr: TokenStream, item: TokenStream) -> TokenStream {
	async_trait::async_trait(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn async_trait_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
	async_trait::async_trait_impl(attr.into(), item.into()).into()
}

#[proc_macro]
pub fn wrapper_functions(item: TokenStream) -> TokenStream {
	wrap_function::wrapper_functions(item.into()).into()
}
