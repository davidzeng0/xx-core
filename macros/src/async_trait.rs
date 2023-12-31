use super::*;
use crate::{
	closure::{
		async_fn::*,
		make_closure::RemoveRefMut,
		transform::{Function, MaybeImplOrFn}
	},
	wrap_function::get_pats
};

fn format_fn_ident(ident: &Ident) -> Ident {
	format_ident!("async_trait_{}", ident)
}

fn format_trait_ident(ident: &Ident) -> Ident {
	format_ident!("{}Ext", ident)
}

fn trait_ext(item: &ItemTrait) -> TokenStream {
	let mut item = item.clone();
	let mut generics = item.generics.clone();

	for generic in &mut generics.params {
		match generic {
			GenericParam::Lifetime(ltg) => ltg.bounds.clear(),
			GenericParam::Type(tg) => tg.bounds.clear(),
			GenericParam::Const(cg) => {
				let ident = &cg.ident;

				*generic = parse_quote! { #ident };
			}
		}
	}

	let ident = &item.ident;
	let supertrait: TypeParamBound = parse_quote_spanned! { ident.span() => #ident #generics };

	item.supertraits.push(supertrait.clone());
	item.ident = format_trait_ident(ident);

	let mut trait_items = Vec::new();

	for trait_item in item.items {
		let TraitItem::Fn(mut func) = trait_item else {
			continue;
		};

		let mut args: Punctuated<Expr, Token![,]> = get_pats(&func.sig)
			.iter()
			.map(|pat| -> Expr {
				let mut pat = pat.clone();

				RemoveRefMut {}.visit_pat_mut(&mut pat);

				parse_quote! { #pat }
			})
			.collect();

		if func.sig.asyncness.is_some() {
			args.push(parse_quote! {
				xx_core::coroutines::get_context().await
			});
		}

		let ident = format_fn_ident(&func.sig.ident);

		if func.sig.receiver().is_some() {
			func.default = Some(parse_quote! {{
				self.#ident (#args)
			}});
		} else {
			func.default = Some(parse_quote! {{
				Self::#ident (#args)
			}});
		}

		func.attrs.push(parse_quote! { #[inline(always) ]});

		RemoveRefMut {}.visit_signature_mut(&mut func.sig);

		if func.sig.asyncness.is_some() {
			if let Err(err) = transform_typed_closure(&mut Function {
				is_item_fn: false,
				attrs: &mut func.attrs,
				env_generics: Some(&item.generics),
				sig: &mut func.sig,
				block: func.default.as_mut()
			}) {
				return err.to_compile_error();
			}
		}

		trait_items.push(TraitItem::Fn(func));
	}

	item.items = trait_items;

	let mut new_generics = item.generics.clone();
	let ident = &item.ident;
	let thistrait = quote_spanned! { ident.span() => #ident #generics };

	new_generics
		.params
		.push(parse_quote! { T: ?Sized + #supertrait });

	quote! {
		#item

		impl #new_generics #thistrait for T {}
	}
}

pub fn async_trait(_: TokenStream, item: TokenStream) -> TokenStream {
	let mut item: ItemTrait = match parse2(item) {
		Ok(item) => item,
		Err(err) => return err.to_compile_error()
	};

	let ext = trait_ext(&item);

	for trait_item in &mut item.items {
		let TraitItem::Fn(func) = trait_item else {
			continue;
		};

		func.sig.ident = format_fn_ident(&func.sig.ident);

		if func.sig.asyncness.is_none() {
			continue;
		}

		if let Err(err) = transform_no_closure(&mut Function {
			is_item_fn: false,
			attrs: &mut func.attrs,
			env_generics: Some(&item.generics),
			sig: &mut func.sig,
			block: func.default.as_mut()
		}) {
			return err.to_compile_error();
		}
	}

	quote! { #item #ext }
}

fn transform_async_impl_func(
	func: &mut ImplItemFn, env_generics: Option<&Generics>
) -> Result<TokenStream> {
	func.sig.ident = format_fn_ident(&func.sig.ident);

	transform_no_closure(&mut Function {
		is_item_fn: false,
		attrs: &mut func.attrs,
		env_generics,
		sig: &mut func.sig,
		block: Some(&mut func.block)
	})?;

	Ok(quote! { #func })
}

fn transform_async_impl(item: &mut ItemImpl) -> Result<TokenStream> {
	for impl_item in &mut item.items {
		if let ImplItem::Fn(func) = impl_item {
			transform_async_impl_func(func, Some(&item.generics))?;
		}
	}

	Ok(quote! { #item }.into())
}

fn transform(item: TokenStream) -> Result<TokenStream> {
	if let Ok(mut parsed) = parse2::<ImplItemFn>(item.clone()) {
		return transform_async_impl_func(&mut parsed, None);
	}

	if let Ok(mut parsed) = parse2::<ItemImpl>(item.clone()) {
		return transform_async_impl(&mut parsed);
	}

	match parse2::<MaybeImplOrFn>(item.clone()) {
		Ok(_) => Ok(item),
		Err(err) => Err(err)
	}
}

pub fn async_trait_impl(_: TokenStream, item: TokenStream) -> TokenStream {
	match transform(item) {
		Ok(ts) => ts,
		Err(err) => err.to_compile_error()
	}
}
