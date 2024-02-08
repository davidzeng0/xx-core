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
		let mut types = vec![quote! { ::xx_core::task::ReqPtr<#return_type> }];

		if let Some(rec) = func.sig.receiver() {
			let ty = &rec.ty;

			types.insert(0, quote! { #ty });
		}

		let default_cancel_capture = make_tuple_type(types);

		quote! {
			::xx_core::task::closure::CancelClosure<#default_cancel_capture>
		}
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

		cancel.sig.inputs.push(parse_quote! {
			request: ::xx_core::task::ReqPtr<#return_type>
		});

		cancel_closure_type = make_explicit_closure(
			&mut Function {
				is_root: true,
				attrs: &mut cancel.attrs,
				env_generics: func.env_generics,
				sig: &mut cancel.sig,
				block: Some(&mut cancel.block)
			},
			vec![(quote! { () }, quote! { () })],
			quote! { ::xx_core::task::closure::CancelClosure },
			|capture, _| quote! { ::xx_core::task::closure::CancelClosure<#capture> }
		)?;

		let (inputs, output, block) = (&cancel.sig.inputs, &cancel.sig.output, &cancel.block);

		*stmt = parse_quote! { let cancel = | #inputs | #output #block; };

		ReplaceSelf {}.visit_stmt_mut(stmt);

		break;
	}

	func.sig.output = parse_quote! {
		-> ::xx_core::task::Progress<#return_type, #cancel_closure_type>
	};

	make_opaque_closure(
		func,
		vec![(
			quote! { request },
			quote! { ::xx_core::task::ReqPtr<#return_type> }
		)],
		|_| quote! { ::xx_core::task::Progress<#return_type, #cancel_closure_type> },
		OpaqueClosureType::Custom(|_| {
			(
				quote! { ::xx_core::task::Task<Output = #return_type, Cancel = #cancel_closure_type> },
				quote! { ::xx_core::task::closure::TaskClosureWrap }
			)
		})
	)?;

	Ok(())
}

pub fn future(_: TokenStream, item: TokenStream) -> TokenStream {
	transform_fn(item, transform_func, |item| match item {
		Functions::Trait(_) | Functions::TraitFn(_) => false,
		_ => true
	})
}
