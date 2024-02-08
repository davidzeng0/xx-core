use syn::parse::Parser;

use super::*;
mod traits;
use traits::*;

struct ReplaceAwait;

impl VisitMut for ReplaceAwait {
	fn visit_expr_mut(&mut self, expr: &mut Expr) {
		visit_expr_mut(self, expr);

		let Expr::Await(inner) = expr else { return };
		let (attrs, base) = (&inner.attrs, inner.base.as_ref());

		*expr = parse_quote! {
			#(#attrs)*
			::xx_core::coroutines::Context::run(
				unsafe { __xx_internal_async_context.as_ref() },
				#base
			)
		};
	}

	fn visit_expr_closure_mut(&mut self, _: &mut ExprClosure) {}

	fn visit_macro_mut(&mut self, mac: &mut Macro) {
		if let Ok(mut exprs) =
			Punctuated::<Expr, Token![,]>::parse_terminated.parse2(mac.tokens.clone())
		{
			for expr in &mut exprs {
				self.visit_expr_mut(expr);
			}

			mac.tokens = exprs.to_token_stream();
		}
	}
}

struct ReplaceAsync;

impl VisitMut for ReplaceAsync {
	fn visit_expr_mut(&mut self, expr: &mut Expr) {
		visit_expr_mut(self, expr);

		let Expr::Async(inner) = expr else { return };
		let (attrs, capture, block) = (&inner.attrs, &inner.capture, &mut inner.block);

		ReplaceAwait {}.visit_block_mut(block);

		*expr = parse_quote! {
			#(#attrs)*
			{
				::xx_core::coroutines::closure::OpaqueClosure::new(
					#capture
					|__xx_internal_async_context: ::xx_core::pointer::Ptr<
						::xx_core::coroutines::Context,
					>| #block
				)
			}
		};
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClosureType {
	None,
	Opaque,
	Explicit
}

fn transform_async(func: &mut Function, closure_type: ClosureType) -> Result<()> {
	if func.sig.asyncness.take().is_none() {
		if !func.is_root {
			return Ok(());
		}

		return Err(Error::new(
			func.sig.span(),
			"The `async` keyword is missing from the function declaration"
		));
	}

	if closure_type != ClosureType::None {
		func.attrs
			.push(parse_quote!( #[must_use = "Task does nothing until you `.await` it"] ));
	}

	if let Some(block) = &mut func.block {
		ReplaceAwait {}.visit_block_mut(*block);
		ReplaceAsync {}.visit_block_mut(*block);
	}

	match closure_type {
		ClosureType::None => {
			func.sig.inputs.push(parse_quote! {
				__xx_internal_async_context: ::xx_core::pointer::Ptr<::xx_core::coroutines::Context>
			});
		}

		ClosureType::Opaque => {
			make_opaque_closure(
				func,
				vec![(
					quote! { __xx_internal_async_context },
					quote! { ::xx_core::pointer::Ptr<::xx_core::coroutines::Context> }
				)],
				|rt| rt,
				OpaqueClosureType::Custom(|rt| {
					(
						quote! { ::xx_core::coroutines::Task<Output = #rt> },
						quote! { ::xx_core::coroutines::closure::OpaqueClosure }
					)
				})
			)?;
		}

		ClosureType::Explicit => {
			make_explicit_closure(
				func,
				vec![(
					quote! { __xx_internal_async_context },
					quote! { ::xx_core::pointer::Ptr<::xx_core::coroutines::Context> }
				)],
				quote! { ::xx_core::coroutines::closure::Closure },
				|capture, ret| {
					quote! { ::xx_core::coroutines::closure::Closure<#capture, #ret> }
				}
			)?;
		}
	}

	Ok(())
}

fn try_transform(attrs: TokenStream, item: TokenStream) -> Result<TokenStream> {
	let options = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attrs)?;

	let mut explicit = false;
	let mut trait_impl = false;

	for option in &options {
		let mut success = false;

		loop {
			let Meta::Path(path) = option else { break };

			if path.leading_colon.is_some() ||
				path.segments.len() != 1 ||
				!path.segments[0].arguments.is_none()
			{
				break;
			}

			match path.segments[0].ident.to_string().as_ref() {
				"explicit" => explicit = true,
				"traitfn" => trait_impl = true,
				_ => break
			}

			success = true;

			break;
		}

		if !success {
			return Err(Error::new(option.span(), "Invalid option"));
		}
	}

	if explicit && trait_impl {
		return Err(Error::new(options.span(), "Invalid combination of options"));
	}

	if explicit {
		return Ok(transform_fn(
			item,
			|func| transform_async(func, ClosureType::Explicit),
			|item| match item {
				Functions::Trait(_) | Functions::TraitFn(_) => false,
				_ => true
			}
		));
	}

	let item = parse2::<Functions>(item)?;

	if trait_impl {
		return async_impl(item);
	}

	match &item {
		Functions::Trait(item) => async_trait(item.clone()),

		Functions::Fn(_) => transform_functions(item.clone(), |func| {
			transform_async(func, ClosureType::Opaque)
		}),

		Functions::Impl(imp) => {
			if imp.trait_.is_some() {
				async_impl(item.clone())
			} else {
				transform_functions(item.clone(), |func| {
					transform_async(func, ClosureType::Opaque)
				})
			}
		}

		Functions::TraitFn(_) => Err(Error::new(Span::call_site(), "Unexpected declaration"))
	}
}

pub fn asynchronous(attrs: TokenStream, item: TokenStream) -> TokenStream {
	match try_transform(attrs, item) {
		Ok(ts) => ts,
		Err(err) => err.to_compile_error()
	}
}
