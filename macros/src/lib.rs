use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::{
	parse::{Parse, ParseStream},
	punctuated::Punctuated,
	spanned::Spanned,
	visit_mut::*,
	*
};

mod async_trait;
mod closure;
mod wrap_function;

#[proc_macro_attribute]
pub fn sync_task(
	attr: proc_macro::TokenStream, item: proc_macro::TokenStream
) -> proc_macro::TokenStream {
	closure::sync_task(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn async_fn(
	attr: proc_macro::TokenStream, item: proc_macro::TokenStream
) -> proc_macro::TokenStream {
	closure::async_fn(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn async_fn_typed(
	attr: proc_macro::TokenStream, item: proc_macro::TokenStream
) -> proc_macro::TokenStream {
	closure::async_fn_typed(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn async_trait(
	attr: proc_macro::TokenStream, item: proc_macro::TokenStream
) -> proc_macro::TokenStream {
	async_trait::async_trait(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn async_trait_impl(
	attr: proc_macro::TokenStream, item: proc_macro::TokenStream
) -> proc_macro::TokenStream {
	async_trait::async_trait_impl(attr.into(), item.into()).into()
}

#[proc_macro]
pub fn wrapper_functions(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	wrap_function::wrapper_functions(item.into()).into()
}
