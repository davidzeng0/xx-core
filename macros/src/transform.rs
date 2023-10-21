use proc_macro::TokenStream;
use quote::quote;
use syn::*;

pub type TransformCallback = fn(
	is_item_fn: bool,
	&mut Vec<Attribute>,
	env_generics: Option<&mut Generics>,
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

fn transform_trait_func(
	func: &mut TraitItemFn, env_generics: Option<&mut Generics>, callback: TransformCallback
) -> Result<TokenStream> {
	callback(
		false,
		&mut func.attrs,
		env_generics,
		&mut func.sig,
		func.default.as_mut()
	)?;

	Ok(quote! { #func }.into())
}

fn transform_impl_func(
	func: &mut ImplItemFn, env_generics: Option<&mut Generics>, callback: TransformCallback
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

fn transform_trait(item: &mut ItemTrait, callback: TransformCallback) -> Result<TokenStream> {
	for trait_item in &mut item.items {
		if let TraitItem::Fn(func) = trait_item {
			transform_trait_func(func, Some(&mut item.generics), callback)?;
		}
	}

	Ok(quote! { #item }.into())
}

fn transform_impl(item: &mut ItemImpl, callback: TransformCallback) -> Result<TokenStream> {
	for impl_item in &mut item.items {
		if let ImplItem::Fn(func) = impl_item {
			transform_impl_func(func, Some(&mut item.generics), callback)?;
		}
	}

	Ok(quote! { #item }.into())
}

pub fn transform_fn(item: TokenStream, callback: TransformCallback) -> Result<TokenStream> {
	if let Ok(mut parsed) = parse::<ItemFn>(item.clone()) {
		return transform_item_func(&mut parsed, callback);
	}

	if let Ok(mut parsed) = parse::<TraitItemFn>(item.clone()) {
		return transform_trait_func(&mut parsed, None, callback);
	}

	if let Ok(mut parsed) = parse::<ImplItemFn>(item.clone()) {
		return transform_impl_func(&mut parsed, None, callback);
	}

	if let Ok(mut parsed) = parse::<ItemTrait>(item.clone()) {
		return transform_trait(&mut parsed, callback);
	}

	if let Ok(mut parsed) = parse::<ItemImpl>(item.clone()) {
		return transform_impl(&mut parsed, callback);
	}

	Err(Error::new_spanned::<proc_macro2::TokenStream, &str>(
		item.into(),
		"Expected a function, trait, or impl"
	))
}
