use proc_macro::TokenStream;

mod async_fn;
mod closure;
mod sync_task;
mod transform;

#[proc_macro_attribute]
pub fn sync_task(attr: TokenStream, item: TokenStream) -> TokenStream {
	sync_task::sync_task(attr, item)
}

#[proc_macro_attribute]
pub fn async_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
	async_fn::async_fn(attr, item)
}

#[proc_macro_attribute]
pub fn async_fn_typed(attr: TokenStream, item: TokenStream) -> TokenStream {
	async_fn::async_fn_typed(attr, item)
}

#[proc_macro_attribute]
pub fn async_fn_no_closure(attr: TokenStream, item: TokenStream) -> TokenStream {
	async_fn::async_fn_no_closure(attr, item)
}

#[proc_macro_attribute]
pub fn async_fn_full(attr: TokenStream, item: TokenStream) -> TokenStream {
	async_fn::async_fn_full(attr, item)
}
