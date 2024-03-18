use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::{
	parse::{Parse, ParseStream, Parser},
	punctuated::Punctuated,
	spanned::Spanned,
	visit_mut::*,
	*
};
use xx_macro_support::{attribute::*, function::*, macro_expr::*};

mod asynchronous;
mod duration;
mod error;
mod future;
mod make_closure;
mod syscall;
mod transform;
mod wrap_function;

use make_closure::*;
use transform::*;

#[proc_macro_attribute]
pub fn asynchronous(
	attr: proc_macro::TokenStream, item: proc_macro::TokenStream
) -> proc_macro::TokenStream {
	asynchronous::asynchronous(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn future(
	attr: proc_macro::TokenStream, item: proc_macro::TokenStream
) -> proc_macro::TokenStream {
	future::future(attr.into(), item.into()).into()
}

#[proc_macro]
pub fn wrapper_functions(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	wrap_function::wrapper_functions(item.into()).into()
}

#[proc_macro]
pub fn syscall_impl(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	syscall::syscall_impl(item.into()).into()
}

#[proc_macro_attribute]
pub fn syscall_define(
	attr: proc_macro::TokenStream, item: proc_macro::TokenStream
) -> proc_macro::TokenStream {
	syscall::syscall_define(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn compact_error(
	attr: proc_macro::TokenStream, item: proc_macro::TokenStream
) -> proc_macro::TokenStream {
	error::compact_error(attr.into(), item.into()).into()
}

#[proc_macro]
pub fn duration(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	duration::duration(item.into()).into()
}
