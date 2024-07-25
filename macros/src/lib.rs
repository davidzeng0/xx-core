use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit_mut::*;
use syn::*;
use xx_macro_support::attribute::*;
use xx_macro_support::fallible::*;
use xx_macro_support::function::*;
use xx_macro_support::impls::*;
use xx_macro_support::visit_macro::*;
use xx_macros::*;

mod asynchronous;
mod duration;
mod error;
mod future;
mod make_closure;
mod syscall;
mod transform;
mod visit;
mod wrap_function;

use self::make_closure::*;
use self::transform::*;
use self::visit::*;

#[proc_macro_attribute]
pub fn asynchronous(
	attr: proc_macro::TokenStream, item: proc_macro::TokenStream
) -> proc_macro::TokenStream {
	asynchronous::asynchronous(attr.into(), item.into()).into()
}

#[proc_macro]
pub fn select(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	asynchronous::branch::select(item.into()).into()
}

#[proc_macro]
pub fn join(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	asynchronous::branch::join(item.into()).into()
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

#[proc_macro]
pub fn duration(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	duration::duration(item.into()).into()
}

#[proc_macro_attribute]
pub fn errors(
	attr: proc_macro::TokenStream, item: proc_macro::TokenStream
) -> proc_macro::TokenStream {
	error::error(attr.into(), item.into()).into()
}
