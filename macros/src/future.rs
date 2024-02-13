use super::*;

fn transform_func(func: &mut Function) -> Result<()> {
	if !func.is_root {
		if let Some(index) = func.attrs.iter().position(|attr| match &attr.meta {
			Meta::Path(path) => path
				.segments
				.last()
				.is_some_and(|last| last.ident == "future" && last.arguments.is_none()),
			_ => false
		}) {
			func.attrs.remove(index);
		} else {
			return Ok(());
		}
	}

	func.attrs.push(parse_quote!( #[must_use] ));

	let return_type = get_return_type(&func.sig.output);

	let mut cancel_closure_type = {
		let mut types = vec![quote! { ::xx_core::future::ReqPtr<#return_type> }];

		if let Some(rec) = func.sig.receiver() {
			let ty = &rec.ty;

			types.insert(0, quote! { #ty });
		}

		let default_cancel_capture = make_tuple_type(types);

		quote! { ::xx_core::future::closure::CancelClosure<#default_cancel_capture> }
	};

	loop {
		let Some(block) = &mut func.block else {
			break;
		};

		let Some(stmt) = (*block).stmts.first_mut() else {
			break;
		};

		let Stmt::Item(Item::Fn(cancel)) = stmt else {
			break;
		};

		if cancel.sig.ident != "cancel" {
			break;
		}

		let Visibility::Inherited = cancel.vis else {
			return Err(Error::new(cancel.vis.span(), "Visibility not allowed here"));
		};

		if let Some(constness) = &cancel.sig.constness {
			return Err(Error::new(constness.span(), "`const` not allowed here"));
		}

		if let Some(asyncness) = &cancel.sig.asyncness {
			return Err(Error::new(asyncness.span(), "`async` not allowed here"));
		}

		if let Some(abi) = &cancel.sig.abi {
			return Err(Error::new(abi.span(), "ABI not allowed here"));
		}

		if let Some(generics) = &cancel.sig.generics.lt_token {
			return Err(Error::new(generics.span(), "Generics not allowed here"));
		}

		if let Some(variadic) = &cancel.sig.variadic {
			return Err(Error::new(variadic.span(), "Variadics not allowed here"));
		}

		cancel.sig.inputs.push(parse_quote! {
			request: ::xx_core::future::ReqPtr<#return_type>
		});

		let attrs = cancel.attrs.clone();

		cancel_closure_type = make_explicit_closure(
			&mut Function {
				is_root: true,
				attrs: &mut cancel.attrs,
				env_generics: func.env_generics,
				sig: &mut cancel.sig,
				block: Some(&mut cancel.block)
			},
			vec![(quote! { () }, quote! { () })],
			quote! { ::xx_core::future::closure::CancelClosure },
			|capture, _| quote! { ::xx_core::future::closure::CancelClosure<#capture> },
			LifetimeAnnotations::Closure
		)?;

		let (unsafety, inputs, output, block) = (
			&cancel.sig.unsafety,
			&cancel.sig.inputs,
			&cancel.sig.output,
			&cancel.block
		);

		*stmt = parse_quote! {
			#(#attrs)*
			let cancel = | #inputs | #output {
				#unsafety #block
			};
		};

		ReplaceSelf {}.visit_stmt_mut(stmt);

		break;
	}

	func.sig.output = parse_quote! {
		-> ::xx_core::future::Progress<#return_type, #cancel_closure_type>
	};

	make_opaque_closure(
		func,
		vec![(
			quote! { request },
			quote! { ::xx_core::future::ReqPtr<#return_type> }
		)],
		|_| quote! { ::xx_core::future::Progress<#return_type, #cancel_closure_type> },
		OpaqueClosureType::Custom(|_| {
			(
				quote! { ::xx_core::future::Future<Output = #return_type, Cancel = #cancel_closure_type> },
				quote! { ::xx_core::future::closure::FutureClosure }
			)
		}),
		false,
		true
	)?;

	Ok(())
}

pub fn future(_: TokenStream, item: TokenStream) -> TokenStream {
	transform_fn(item, transform_func, |item| match item {
		Functions::Trait(_) | Functions::TraitFn(_) => false,
		_ => true
	})
}
