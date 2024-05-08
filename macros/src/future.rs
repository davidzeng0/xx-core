use super::*;

fn transform_func(func: &mut Function<'_>) -> Result<()> {
	if !func.is_root && remove_attr_path(func.attrs, "future").is_none() {
		return Ok(());
	}

	func.attrs.push(parse_quote!( #[must_use] ));

	let return_type = get_return_type(&func.sig.output);

	let mut cancel_closure_type = {
		let mut types = vec![quote! { ::xx_core::future::ReqPtr<#return_type> }];

		if let Some(rec) = func.sig.receiver() {
			let ty = &rec.ty;

			types.insert(0, quote! { #ty });
		}

		let default_cancel_capture = make_tuple_of_types(types);

		quote! { ::xx_core::future::closure::CancelClosure<#default_cancel_capture> }
	};

	if let Some(block) = &mut func.block {
		fn not_allowed<T>(what: &Option<T>, message: &'static str) -> Result<()>
		where
			T: ToTokens
		{
			if let Some(tokens) = what {
				Err(Error::new_spanned(tokens, message))
			} else {
				Ok(())
			}
		}

		for stmt in &mut block.stmts {
			let Stmt::Item(Item::Fn(cancel)) = stmt else {
				continue;
			};

			if remove_attr_path(&mut cancel.attrs, "cancel").is_none() {
				continue;
			}

			let Visibility::Inherited = cancel.vis else {
				return Err(Error::new_spanned(
					&cancel.vis,
					"Visibility not allowed here"
				));
			};

			not_allowed(&cancel.sig.constness, "`const` not allowed here")?;
			not_allowed(&cancel.sig.asyncness, "`async` not allowed here")?;
			not_allowed(&cancel.sig.abi, "ABI not allowed here")?;
			not_allowed(&cancel.sig.generics.lt_token, "Generics not allowed here")?;
			not_allowed(&cancel.sig.variadic, "Variadics not allowed here")?;

			cancel.sig.inputs.push(parse_quote! {
				request: ::xx_core::future::ReqPtr<#return_type>
			});

			cancel_closure_type = make_explicit_closure(
				&mut Function {
					is_root: true,
					attrs: &mut vec![],
					env_generics: func.env_generics,
					sig: &mut cancel.sig,
					block: Some(&mut cancel.block)
				},
				&[(quote! { () }, quote! { () })],
				quote! { ::xx_core::future::closure::CancelClosure },
				|capture, ret| {
					quote_spanned! { ret.span() =>
						::xx_core::future::closure::CancelClosure<#capture>
					}
				},
				LifetimeAnnotations::Closure
			)?;

			ReplaceSelf {}.visit_item_fn_mut(cancel);

			let (ident, attrs, unsafety, inputs, output, block) = (
				&cancel.sig.ident,
				&cancel.attrs,
				&cancel.sig.unsafety,
				&cancel.sig.inputs,
				&cancel.sig.output,
				&cancel.block
			);

			*stmt = parse_quote_spanned! {
				cancel.span() =>

				#[allow(unused_variables)]
				#(#attrs)*
				let #ident = | #inputs | #output #unsafety #block;
			};

			break;
		}
	}

	func.sig.output = parse_quote_spanned! { return_type.span() =>
		-> ::xx_core::future::Progress<#return_type, #cancel_closure_type>
	};

	make_opaque_closure(
		func,
		&[(
			quote! { request },
			quote! { ::xx_core::future::ReqPtr<#return_type> }
		)],
		|_| {
			quote_spanned! { return_type.span() =>
				::xx_core::future::Progress<#return_type, #cancel_closure_type>
			}
		},
		OpaqueClosureType::Custom(|ret: TokenStream| {
			(
				quote_spanned! { ret.span() =>
					::xx_core::future::Future<Output = #return_type, Cancel = #cancel_closure_type>
				},
				quote! { ::xx_core::future::closure::FutureClosure }
			)
		}),
		LifetimeAnnotations::Auto
	)?;

	Ok(())
}

pub fn future(_: TokenStream, item: TokenStream) -> TokenStream {
	transform_fn(item, transform_func, |item| {
		!matches!(item, Functions::Trait(_) | Functions::TraitFn(_))
	})
}
