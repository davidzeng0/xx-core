use super::*;

fn format_fn_ident(ident: &Ident) -> Ident {
	format_ident!("__xx_async_impl_{}", ident)
}

fn format_trait_ident(ident: &Ident) -> Ident {
	format_ident!("{}Ext", ident)
}

fn trait_ext(mut attrs: AttributeArgs, mut ext: ItemTrait) -> Result<TokenStream> {
	let name = ext.ident.clone();

	ext.ident = format_trait_ident(&name);

	let (_, type_generics, where_clause) = ext.generics.split_for_impl();
	let super_trait: TypeParamBound = parse_quote! { #name #type_generics };

	ext.supertraits.push(super_trait.clone());
	attrs.async_kind.0 = AsyncKind::TraitExt;

	for trait_item in take(&mut ext.items) {
		let TraitItem::Fn(mut func) = trait_item else {
			continue;
		};

		let mut call = Vec::new();
		let ident = format_fn_ident(&func.sig.ident);

		call.push(quote! { <Self as #super_trait>::#ident });

		if !func.sig.generics.params.is_empty() {
			let mut generics = func.sig.generics.clone();

			generics.params = generics
				.params
				.into_iter()
				.filter(|generic| !matches!(generic, GenericParam::Lifetime(_)))
				.collect();
			let (_, type_generics, _) = generics.split_for_impl();

			call.push(type_generics.as_turbofish().into_token_stream());
		}

		let mut args = get_args(&func.sig.inputs, true);

		if func.sig.asyncness.is_some() {
			let context = Context::ident();

			args.push(parse_quote! { #context });
		}

		call.push(quote! { (#args) });
		func.default = Some(parse_quote! {{ #(#call)* }});

		RemoveModifiers {}.visit_signature_mut(&mut func.sig);

		if func.sig.asyncness.is_some() {
			transform_async(
				attrs.clone(),
				&mut Function::from_trait_fn(false, Some(&ext.generics), &mut func)
			)?;
		}

		func.attrs.push(parse_quote! {
			#[allow(unsafe_op_in_unsafe_fn, clippy::used_underscore_binding)]
		});

		ext.items.push(TraitItem::Fn(func));
	}

	let ident = &ext.ident;
	let mut new_generics = ext.generics.clone();

	new_generics.params.push(parse_quote! {
		XXInternalTraitImplementer: #super_trait + ?Sized
	});

	Ok(quote! {
		#[cfg(not(doc))]
		#ext

		#[cfg(not(doc))]
		impl #new_generics #ident #type_generics for XXInternalTraitImplementer #where_clause {}
	})
}

pub fn async_trait(mut attrs: AttributeArgs, item: ItemTrait) -> Result<TokenStream> {
	let ext = trait_ext(attrs.clone(), item.clone())?;

	attrs.async_kind.0 = AsyncKind::TraitFn;

	let functions = Functions::Trait(item);
	let item = functions.transform_all(
		|func| {
			func.is_root = false;
			func.sig.ident = format_fn_ident(&func.sig.ident);

			transform_async(attrs.clone(), func)
		},
		|_| true
	)?;

	Ok(quote! { #item #ext })
}

pub fn async_impl(mut attrs: AttributeArgs, item: Functions) -> Result<TokenStream> {
	attrs.async_kind.0 = AsyncKind::TraitFn;

	item.transform_all(
		|func| {
			func.is_root = false;
			func.sig.ident = format_fn_ident(&func.sig.ident);

			transform_async(attrs.clone(), func)
		},
		|item| matches!(item, Functions::Fn(_) | Functions::Impl(_))
	)
}
