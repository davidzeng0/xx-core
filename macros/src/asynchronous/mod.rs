use std::mem::take;

use super::*;
mod traits;
use traits::*;

fn transform_block(block: &mut Block) {
	block.stmts.insert(
		0,
		parse_quote! {
			let __xx_internal_async_context = unsafe {
				::xx_core::pointer::Ptr::<
					::xx_core::coroutines::Context
				>::as_ref(__xx_internal_async_context)
			};
		}
	);
}

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

		transform_block(&mut block);

		parse_quote_spanned! {
			inner.span() =>
			#(#attrs)*
			::xx_core::coroutines::closure::OpaqueClosure::new(
				#capture
				|__xx_internal_async_context: ::xx_core::pointer::Ptr<
					::xx_core::coroutines::Context,
				>| #block
			)
		}
	}

	fn transform_await(&mut self, inner: &mut ExprAwait) -> Expr {
		self.visit_expr_await_mut(inner);

		let (attrs, base) = (&inner.attrs, inner.base.as_ref());

		parse_quote_spanned! {
			inner.span() =>
			#(#attrs)*
			::xx_core::coroutines::Context::run(
				__xx_internal_async_context,
				#base
			)
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ClosureType {
	None,
	Opaque,
	Explicit,
	OpaqueTrait
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Lang {
	GetContext
}

fn transform_async(func: &mut Function<'_>, closure_type: ClosureType) -> Result<()> {
	if func.sig.asyncness.take().is_none() {
		return if !func.is_root {
			Ok(())
		} else {
			Err(Error::new_spanned(
				func.sig.fn_token,
				"The `async` keyword is missing from the function declaration"
			))
		};
	}

	let mut lang = None;

	if let Some(value) = remove_attr_name_value(func.attrs, "lang") {
		let Expr::Lit(ExprLit { lit: Lit::Str(str), .. }) = value else {
			return Err(Error::new_spanned(value, "Expected a str"));
		};

		match str.value().as_ref() {
			"get_context" => lang = Some(Lang::GetContext),
			_ => return Err(Error::new_spanned(str, "Unknown lang"))
		}
	}

	if closure_type != ClosureType::None {
		func.attrs
			.push(parse_quote!( #[must_use = "Task does nothing until you `.await` it"] ));
	}

	match (lang, &mut func.block) {
		(None, Some(block)) => {
			transform_block(block);

			TransformAsync {}.visit_block_mut(block);
		}

		(Some(_), None) => return Err(Error::new_spanned(&func.sig, "An empty block is required")),
		(Some(_), Some(block)) if !block.stmts.is_empty() => {
			return Err(Error::new_spanned(block, "This block must be empty"))
		}

		(Some(Lang::GetContext), Some(block)) => block.stmts.push(Stmt::Expr(
			parse_quote! { __xx_internal_async_context },
			None
		)),

		(None, None) => ()
	}

	let context_ident = quote! { __xx_internal_async_context };
	let context_type = quote! { ::xx_core::pointer::Ptr<::xx_core::coroutines::Context> };
	let args = vec![(context_ident.clone(), context_type.clone())];

	match closure_type {
		ClosureType::None => {
			func.sig
				.inputs
				.push(parse_quote! { #context_ident: #context_type });
		}

		ClosureType::Opaque | ClosureType::OpaqueTrait => {
			make_opaque_closure(
				func,
				&args,
				|rt| rt,
				OpaqueClosureType::Custom(|rt: TokenStream| {
					(
						quote_spanned! { rt.span() => ::xx_core::coroutines::Task<Output = #rt> },
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
				&args,
				quote! { ::xx_core::coroutines::closure::Closure },
				|capture, ret| {
					quote_spanned! { ret.span() => ::xx_core::coroutines::closure::Closure<#capture, #ret> }
				},
				LifetimeAnnotations::Auto
			)?;
		}
	}

	Ok(())
}

fn parse_attrs(attrs: TokenStream) -> Result<Option<ClosureType>> {
	let options = Punctuated::<Ident, Token![,]>::parse_terminated.parse2(attrs)?;
	let mut closure_type = None;

	for option in &options {
		if closure_type.is_some() {
			return Err(Error::new_spanned(
				options,
				"Invalid combination of options"
			));
		}

		match option.to_string().as_ref() {
			"explicit" => closure_type = Some(ClosureType::Explicit),
			"traitfn" => closure_type = Some(ClosureType::None),
			"traitext" => closure_type = Some(ClosureType::OpaqueTrait),
			_ => return Err(Error::new_spanned(option, "Unknown option"))
		}
	}

	Ok(closure_type)
}

fn try_transform(attrs: TokenStream, item: TokenStream) -> Result<TokenStream> {
	let closure_type = parse_attrs(attrs)?;
	let item = parse2::<Functions>(item)?;

	let transform_functions = |ty: ClosureType| {
		transform_functions(
			item.clone(),
			|func| transform_async(func, ty),
			|item| !matches!(item, Functions::Trait(_) | Functions::TraitFn(_))
		)
	};

	if let Some(ty) = &closure_type {
		return if *ty == ClosureType::None {
			async_impl(item)
		} else {
			transform_functions(*ty)
		};
	}

	match &item {
		Functions::Trait(item) => async_trait(item.clone()),
		Functions::Impl(imp) if imp.trait_.is_some() => async_impl(item.clone()),
		Functions::Fn(_) | Functions::Impl(_) => transform_functions(ClosureType::Opaque),
		Functions::TraitFn(_) => {
			let message = "Trait functions must specify `#[asynchronous(traitfn)]`";

			Err(Error::new(Span::call_site(), message))
		}
	}
}

pub fn asynchronous(attrs: TokenStream, item: TokenStream) -> TokenStream {
	match try_transform(attrs, item) {
		Ok(ts) => ts,
		Err(err) => err.to_compile_error()
	}
}
