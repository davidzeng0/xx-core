use proc_macro2::TokenStream;
use quote::quote;
use syn::*;

use crate::async_trait::MaybeImplOrFn;

pub type TransformCallback = fn(
	is_item_fn: bool,
	&mut Vec<Attribute>,
	env_generics: Option<&Generics>,
	&mut Signature,
	Option<&mut Block>
) -> Result<()>;

fn transform_item_func(func: &mut ItemFn, callback: TransformCallback) -> Result<TokenStream> {
	callback(
		true,
		&mut func.attrs,
		None,
		&mut func.sig,
		Some(func.block.as_mut())
	)?;

	Ok(quote! { #func }.into())
}

fn transform_impl_func(
	func: &mut ImplItemFn, env_generics: Option<&Generics>, callback: TransformCallback
) -> Result<TokenStream> {
	callback(
		false,
		&mut func.attrs,
		env_generics,
		&mut func.sig,
		Some(&mut func.block)
	)?;

	Ok(quote! { #func }.into())
}

fn transform_impl(item: &mut ItemImpl, callback: TransformCallback) -> Result<TokenStream> {
	for impl_item in &mut item.items {
		if let ImplItem::Fn(func) = impl_item {
			transform_impl_func(func, Some(&item.generics), callback)?;
		}
	}

	Ok(quote! { #item }.into())
}

pub fn transform_fn(item: TokenStream, callback: TransformCallback) -> Result<TokenStream> {
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
