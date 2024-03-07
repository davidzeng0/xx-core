use std::mem::take;

use super::*;
mod traits;
use traits::*;

struct HasAsync(bool);

impl VisitMut for HasAsync {
	fn visit_item_mut(&mut self, _: &mut Item) {}

	fn visit_expr_closure_mut(&mut self, _: &mut ExprClosure) {}

	fn visit_expr_async_mut(&mut self, _: &mut ExprAsync) {
		self.0 = true;
	}

	fn visit_macro_mut(&mut self, mac: &mut Macro) {
		visit_macro_punctuated_exprs(self, mac);
	}
}

struct TransformAsync;

impl TransformAsync {
	fn transform_async(&mut self, inner: &mut ExprAsync) -> Expr {
		self.visit_expr_async_mut(inner);

		let (attrs, capture, mut block) = (&inner.attrs, &inner.capture, inner.block.clone());

		block.stmts.insert(
			0,
			parse_quote! {
				let __xx_internal_async_context = unsafe { __xx_internal_async_context.as_ref() };
			}
		);

		parse_quote_spanned! {
			inner.span() => {
				#(#attrs)*
				::xx_core::coroutines::closure::OpaqueClosure::new(
					#capture
					|__xx_internal_async_context: ::xx_core::pointer::Ptr<
						::xx_core::coroutines::Context,
					>| #block
				)
			}
		}
	}

	fn transform_await(&mut self, inner: &mut ExprAwait) -> Expr {
		self.visit_expr_await_mut(inner);

		let (attrs, base) = (&inner.attrs, inner.base.as_ref());

		parse_quote_spanned! {
			inner.span() => {
				#(#attrs)*
				::xx_core::coroutines::Context::run(
					__xx_internal_async_context,
					#base
				)
			}
		}
	}

	fn transform_closure(&mut self, closure: &mut ExprClosure) -> Expr {
		let asyncness = closure.asyncness.take();
		let body = closure.body.as_mut();

		if let Some(asyncness) = &asyncness {
			*body = parse_quote_spanned! { asyncness.span() => #asyncness move { #body } };
		} else {
			let mut has_async = HasAsync(false);

			has_async.visit_expr_mut(body);

			if !has_async.0 {
				return Expr::Closure(closure.clone());
			}
		}

		self.visit_expr_mut(body);

		Expr::Closure(closure.clone())
	}
}

impl VisitMut for TransformAsync {
	fn visit_expr_mut(&mut self, expr: &mut Expr) {
		*expr = match expr {
			Expr::Async(inner) => self.transform_async(inner),
			Expr::Await(inner) => self.transform_await(inner),
			Expr::Closure(inner) => self.transform_closure(inner),
			_ => return visit_expr_mut(self, expr)
		};
	}

	fn visit_macro_mut(&mut self, mac: &mut Macro) {
		visit_macro_punctuated_exprs(self, mac);
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClosureType {
	None,
	Opaque,
	Explicit,
	OpaqueTrait
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
		block.stmts.insert(
			0,
			parse_quote! {
				let __xx_internal_async_context = unsafe { __xx_internal_async_context.as_ref() };
			}
		);

		TransformAsync {}.visit_block_mut(*block);
	}

	match closure_type {
		ClosureType::None => {
			func.sig.inputs.push(parse_quote! {
				__xx_internal_async_context: ::xx_core::pointer::Ptr<::xx_core::coroutines::Context>
			});
		}

		ClosureType::Opaque | ClosureType::OpaqueTrait => {
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
				}),
				closure_type == ClosureType::OpaqueTrait,
				closure_type == ClosureType::Opaque
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
				},
				LifetimeAnnotations::Auto
			)?;
		}
	}

	Ok(())
}

fn parse_attrs(attrs: TokenStream) -> Result<Option<ClosureType>> {
	let options = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attrs)?;
	let mut closure_type = None;

	for option in &options {
		let mut success = false;

		loop {
			let Meta::Path(path) = option else { break };
			let Some(ident) = path.get_ident() else { break };

			if closure_type.is_some() {
				break;
			}

			match ident.to_string().as_ref() {
				"explicit" => closure_type = Some(ClosureType::Explicit),
				"traitfn" => closure_type = Some(ClosureType::None),
				"traitext" => closure_type = Some(ClosureType::OpaqueTrait),
				_ => return Err(Error::new_spanned(option, "Unknown option"))
			}

			success = true;

			break;
		}

		if !success {
			return Err(Error::new_spanned(
				options,
				"Invalid combination of options"
			));
		}
	}

	Ok(closure_type)
}

fn try_transform(attrs: TokenStream, item: TokenStream) -> Result<TokenStream> {
	let closure_type = parse_attrs(attrs)?;
	let item = parse2::<Functions>(item)?;

	match closure_type {
		Some(closure_type @ (ClosureType::OpaqueTrait | ClosureType::Explicit)) => {
			return transform_functions(
				item,
				|func| transform_async(func, closure_type),
				|item| match item {
					Functions::Trait(_) | Functions::TraitFn(_) => false,
					_ => true
				}
			);
		}

		Some(ClosureType::None) => {
			return async_impl(item);
		}

		_ => ()
	}

	match &item {
		Functions::Trait(item) => async_trait(item.clone()),

		Functions::Fn(_) => transform_functions(
			item.clone(),
			|func| transform_async(func, ClosureType::Opaque),
			|_| true
		),

		Functions::Impl(imp) => {
			if imp.trait_.is_some() {
				async_impl(item.clone())
			} else {
				transform_functions(
					item.clone(),
					|func| transform_async(func, ClosureType::Opaque),
					|_| true
				)
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
