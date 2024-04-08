use super::*;

fn format_fn_ident(ident: &Ident) -> Ident {
	format_ident!("async_trait_{}", ident)
}

fn format_trait_ident(ident: &Ident) -> Ident {
	format_ident!("{}Ext", ident)
}

fn remove_bounds(mut generics: Generics) -> Generics {
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

	generics
}

fn trait_ext(mut ext: ItemTrait) -> Result<TokenStream> {
	let name = ext.ident.clone();

	ext.ident = format_trait_ident(&name);

	let generics = remove_bounds(ext.generics.clone());
	let super_trait: TypeParamBound = parse_quote_spanned! { name.span() => #name #generics };

	ext.supertraits.push(super_trait.clone());

	for trait_item in take(&mut ext.items) {
		let TraitItem::Fn(mut func) = trait_item else {
			continue;
		};

		let mut call = Vec::new();
		let ident = format_fn_ident(&func.sig.ident);

		call.push(quote_spanned! { func.sig.span() => <Self as #super_trait>::#ident });

		if !func.sig.generics.params.is_empty() {
			call.push(quote! { :: });
			call.push(remove_bounds(func.sig.generics.clone()).to_token_stream());
		}

		let mut args = get_args(&func.sig.inputs, true);

		if func.sig.asyncness.is_some() {
			args.push(parse_quote_spanned! { func.sig.inputs.span() =>
				::xx_core::coroutines::get_context().await
			});
		}

		call.push(quote_spanned! { args.span() => (#args) });
		func.default = Some(parse_quote! {{ #(#call)* }});

		RemoveModifiers {}.visit_signature_mut(&mut func.sig);

		if func.sig.asyncness.is_some() {
			transform_async(
				&mut Function::from_trait_fn(false, Some(&ext.generics), &mut func),
				ClosureType::OpaqueTrait
			)?;
		}

		func.attrs
			.push(parse_quote! { #[allow(unsafe_op_in_unsafe_fn)] });
		ext.items.push(TraitItem::Fn(func));
	}

	let ident = &ext.ident;
	let mut new_generics = ext.generics.clone();

	new_generics
		.params
		.push(parse_quote! { XXInternalTraitImplementer: #super_trait + ?Sized });
	Ok(quote! {
		#[cfg(not(doc))]
		#ext

		#[cfg(not(doc))]
		impl #new_generics #ident #generics for XXInternalTraitImplementer {}
	})
}

pub fn async_trait(item: ItemTrait) -> Result<TokenStream> {
	let ext = trait_ext(item.clone())?;

	let functions = Functions::Trait(item);
	let item = functions.transform_all(
		|func| {
			func.sig.ident = format_fn_ident(&func.sig.ident);

			if func.sig.asyncness.is_some() {
				transform_async(func, ClosureType::None)
			} else {
				Ok(())
			}
		},
		|_| true
	)?;

	Ok(quote! { #item #ext })
}

pub fn async_impl(item: Functions) -> Result<TokenStream> {
	item.transform_all(
		|func| {
			func.is_root = false;
			func.sig.ident = format_fn_ident(&func.sig.ident);

			transform_async(func, ClosureType::None)
		},
		|item| matches!(item, Functions::Fn(_) | Functions::Impl(_))
	)
}
