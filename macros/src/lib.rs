use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit_mut::*;
use xx_macro_support::*;
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

declare_attribute_macro! {
	pub fn asynchronous(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
		asynchronous::asynchronous(attr, item)
	}
}

declare_proc_macro! {
	pub fn select(item: TokenStream) -> Result<TokenStream> {
		asynchronous::branch::select(item)
	}
}

declare_proc_macro! {
	pub fn join(item: TokenStream) -> Result<TokenStream> {
		asynchronous::branch::join(item)
	}
}

declare_attribute_macro! {
	pub fn future(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
		future::future(attr, item)
	}
}

declare_proc_macro! {
	pub fn wrapper_functions(item: TokenStream) -> Result<TokenStream> {
		wrap_function::wrapper_functions(item)
	}
}

declare_proc_macro! {
	pub fn syscall_impl(item: TokenStream) -> Result<TokenStream> {
		syscall::syscall_impl(item)
	}
}

declare_attribute_macro! {
	pub fn syscall_define(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
		syscall::syscall_define(attr, item)
	}
}

declare_proc_macro! {
	pub fn duration(item: TokenStream) -> Result<TokenStream> {
		duration::duration(item)
	}
}

declare_attribute_macro! {
	pub fn errors(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
		error::errors(attr, item)
	}
}
