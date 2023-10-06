use proc_macro::{Span, TokenStream};
use quote::quote;
use syn::*;

pub type TransformCallback =
	fn(is_item_fn: bool, &mut Vec<Attribute>, &mut Signature, Option<&mut Block>) -> Result<()>;

fn transform_item_func(func: &mut ItemFn, callback: TransformCallback) -> Result<TokenStream> {
	callback(
		true,
		&mut func.attrs,
		&mut func.sig,
		Some(func.block.as_mut())
	)?;

	Ok(quote! { #func }.into())
}

fn transform_trait_func(
	func: &mut TraitItemFn, callback: TransformCallback
) -> Result<TokenStream> {
	callback(false, &mut func.attrs, &mut func.sig, func.default.as_mut())?;

	Ok(quote! { #func }.into())
}

fn transform_impl_func(func: &mut ImplItemFn, callback: TransformCallback) -> Result<TokenStream> {
	callback(false, &mut func.attrs, &mut func.sig, Some(&mut func.block))?;

	Ok(quote! { #func }.into())
}

fn transform_trait(item: &mut ItemTrait, callback: TransformCallback) -> Result<TokenStream> {
	for trait_item in &mut item.items {
		if let TraitItem::Fn(func) = trait_item {
			transform_trait_func(func, callback)?;
		}
	}

	Ok(quote! { #item }.into())
}

fn transform_impl(item: &mut ItemImpl, callback: TransformCallback) -> Result<TokenStream> {
	for impl_item in &mut item.items {
		if let ImplItem::Fn(func) = impl_item {
			transform_impl_func(func, callback)?;
		}
	}

	Ok(quote! { #item }.into())
}

pub fn transform_fn(item: TokenStream, callback: TransformCallback) -> Result<TokenStream> {
	if let Ok(mut parsed) = parse::<ItemFn>(item.clone()) {
		return transform_item_func(&mut parsed, callback);
	}

	if let Ok(mut parsed) = parse::<TraitItemFn>(item.clone()) {
		return transform_trait_func(&mut parsed, callback);
	}

	if let Ok(mut parsed) = parse::<ImplItemFn>(item.clone()) {
		return transform_impl_func(&mut parsed, callback);
	}

	if let Ok(mut parsed) = parse::<ItemTrait>(item.clone()) {
		return transform_trait(&mut parsed, callback);
	}

	if let Ok(mut parsed) = parse::<ItemImpl>(item.clone()) {
		return transform_impl(&mut parsed, callback);
	}

	Err(Error::new(
		item.into_iter()
			.next()
			.map_or_else(Span::call_site, |t| t.span())
			.into(),
		"Expected a function, trait, or impl"
	))
}
