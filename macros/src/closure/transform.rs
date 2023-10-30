use proc_macro2::TokenStream;
use quote::quote;
use syn::*;

use crate::async_trait::MaybeImplOrFn;

pub struct Function<'a> {
	pub is_item_fn: bool,
	pub attrs: &'a mut Vec<Attribute>,
	pub env_generics: Option<&'a Generics>,
	pub sig: &'a mut Signature,
	pub block: Option<&'a mut Block>
}

type Callback = fn(&mut Function) -> Result<()>;

fn transform_item_func(func: &mut ItemFn, callback: Callback) -> Result<TokenStream> {
	callback(&mut Function {
		is_item_fn: true,
		attrs: &mut func.attrs,
		env_generics: None,
		sig: &mut func.sig,
		block: Some(&mut func.block)
	})?;

	Ok(quote! { #func }.into())
}

fn transform_impl_func(
	func: &mut ImplItemFn, env_generics: Option<&Generics>, callback: Callback
) -> Result<TokenStream> {
	callback(&mut Function {
		is_item_fn: false,
		attrs: &mut func.attrs,
		env_generics,
		sig: &mut func.sig,
		block: Some(&mut func.block)
	})?;

	Ok(quote! { #func }.into())
}

fn transform_impl(item: &mut ItemImpl, callback: Callback) -> Result<TokenStream> {
	for impl_item in &mut item.items {
		if let ImplItem::Fn(func) = impl_item {
			transform_impl_func(func, Some(&item.generics), callback)?;
		}
	}

	Ok(quote! { #item }.into())
}

pub fn transform_fn(item: TokenStream, callback: Callback) -> Result<TokenStream> {
	if let Ok(mut parsed) = parse2::<ItemFn>(item.clone()) {
		return transform_item_func(&mut parsed, callback);
	}

	if let Ok(mut parsed) = parse2::<ImplItemFn>(item.clone()) {
		return transform_impl_func(&mut parsed, None, callback);
	}

	if let Ok(mut parsed) = parse2::<ItemImpl>(item.clone()) {
		return transform_impl(&mut parsed, callback);
	}

	match parse2::<MaybeImplOrFn>(item.clone()) {
		Ok(_) => Ok(item),
		Err(err) => Err(err)
	}
}
