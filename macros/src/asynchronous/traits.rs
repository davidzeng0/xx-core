use super::*;

fn format_fn_ident(ident: &Ident) -> Ident {
	format_ident!("async_trait_{}", ident)
}

fn format_trait_ident(ident: &Ident) -> Ident {
	format_ident!("{}Ext", ident)
}

fn trait_ext(item: &ItemTrait) -> Result<TokenStream> {
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

		let ident = format_fn_ident(&func.sig.ident);
		let mut args: Punctuated<Expr, Token![,]> = get_args(&func.sig, true);

		if func.sig.asyncness.is_some() {
			args.push(parse_quote! {
				::xx_core::coroutines::get_context().await
			});
		}

		func.default = Some(parse_quote! {{
			Self::#ident (#args)
		}});

		RemoveRefMut {}.visit_signature_mut(&mut func.sig);

		if func.sig.asyncness.is_some() {
			transform_async(
				&mut Function {
					is_root: false,
					attrs: &mut func.attrs,
					env_generics: Some(&item.generics),
					sig: &mut func.sig,
					block: func.default.as_mut()
				},
				ClosureType::OpaqueTrait
			)?;
		}

		trait_items.push(TraitItem::Fn(func));
	}

	item.items = trait_items;

	let mut new_generics = item.generics.clone();
	let ident = &item.ident;
	let thistrait = quote_spanned! { ident.span() => #ident #generics };

	new_generics
		.params
		.push(parse_quote! { XXInternalTraitImplementer: #supertrait + ?Sized });

	Ok(quote! {
		#item

		impl #new_generics #thistrait for XXInternalTraitImplementer {}
	})
}

pub fn async_trait(mut item: ItemTrait) -> Result<TokenStream> {
	let ext = trait_ext(&item)?;

	for trait_item in &mut item.items {
		let TraitItem::Fn(func) = trait_item else {
			continue;
		};

		func.sig.ident = format_fn_ident(&func.sig.ident);

		if func.sig.asyncness.is_none() {
			continue;
		}

		transform_async(
			&mut Function {
				is_root: false,
				attrs: &mut func.attrs,
				env_generics: Some(&item.generics),
				sig: &mut func.sig,
				block: func.default.as_mut()
			},
			ClosureType::None
		)?;
	}

	Ok(quote! { #item #ext })
}

pub fn async_impl(item: Functions) -> Result<TokenStream> {
	match &item {
		Functions::Fn(_) | Functions::Impl(_) => (),
		_ => return Err(Error::new(Span::call_site(), "Unexpected declaration"))
	}

	transform_functions(item, |func| {
		func.is_root = false;
		func.sig.ident = format_fn_ident(&func.sig.ident);

		transform_async(func, ClosureType::None)
	})
}
