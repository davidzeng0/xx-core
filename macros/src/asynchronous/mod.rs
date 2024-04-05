use std::mem::take;

use super::*;

pub mod branch;
mod traits;

use traits::*;

fn transform_block(block: &mut Block) {
	let index = block
		.stmts
		.iter()
		.position(|stmt| !matches!(stmt, Stmt::Item(_)))
		.unwrap_or(block.stmts.len());

	block.stmts.insert(
		index,
		parse_quote! {
			let __xx_internal_async_context = unsafe {
				::xx_core::pointer::Ptr::<
					::xx_core::coroutines::Context
				>::as_ref(__xx_internal_async_context)
			};
		}
	);
}

fn tuple_args(args: &mut Punctuated<Pat, Token![,]>) {
	let (mut pats, mut tys) = (Vec::new(), Vec::new());

	for input in take(args) {
		match input {
			Pat::Type(ty) => {
				pats.push(ty.pat.as_ref().clone());
				tys.push(ty.ty.as_ref().clone());
			}

			_ => {
				pats.push(input.clone());
				tys.push(parse_quote! { _ });
			}
		}
	}

	let (pats, tys) = (make_tuple_of_types(pats), make_tuple_of_types(tys));

	args.push(Pat::Type(parse_quote! { #pats: #tys }));
}

struct TransformAsync;

impl TransformAsync {
	fn transform_async(&mut self, inner: &mut ExprAsync) -> Expr {
		self.visit_expr_async_mut(inner);

		let (attrs, capture, mut block) = (&inner.attrs, &inner.capture, inner.block.clone());

		transform_block(&mut block);

		parse_quote_spanned! { inner.span() =>
			#(#attrs)*
			::xx_core::coroutines::closure::OpaqueClosure
				::<_, _, { ::xx_core::closure::INLINE_DEFAULT }>
				::new(
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

		parse_quote_spanned! { inner.span() =>
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
		let mut block;

		#[allow(clippy::never_loop)]
		loop {
			if asyncness.is_some() {
				match body {
					Expr::Block(blk) => block = blk.clone(),
					_ => block = parse_quote! {{ #[allow(unused_braces)] #body }}
				}

				break;
			}

			if let Expr::Async(expr @ ExprAsync { capture: Some(_), .. }) = body {
				block = ExprBlock {
					attrs: expr.attrs.clone(),
					label: None,
					block: expr.block.clone()
				};

				break;
			}

			return Expr::Closure(closure.clone());
		}

		self.visit_expr_block_mut(&mut block);

		transform_block(&mut block.block);

		*body = Expr::Block(block);

		tuple_args(&mut closure.inputs);

		closure.inputs.push(Pat::Type(parse_quote! {
			__xx_internal_async_context: ::xx_core::pointer::Ptr<
				::xx_core::coroutines::Context
			>
		}));

		parse_quote_spanned! { closure.span() =>
			::xx_core::impls::internal::OpaqueAsyncFn(#closure)
		}
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
		visit_macro_body(self, mac);
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

fn get_lang(attrs: &mut Vec<Attribute>) -> Result<Option<Lang>> {
	let mut lang = None;

	if let Some(value) = remove_attr_name_value(attrs, "lang") {
		let Expr::Lit(ExprLit { lit: Lit::Str(str), .. }) = value else {
			return Err(Error::new_spanned(value, "Expected a str"));
		};

		match str.value().as_ref() {
			"get_context" => lang = Some(Lang::GetContext),
			_ => return Err(Error::new_spanned(str, "Unknown lang"))
		}
	}

	Ok(lang)
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

	if closure_type != ClosureType::None {
		func.attrs
			.push(parse_quote!( #[must_use = "Task does nothing until you `.await` it"] ));
	}

	match (get_lang(func.attrs)?, &mut func.block) {
		(None, Some(block)) => {
			transform_block(block);

			TransformAsync {}.visit_block_mut(block);
		}

		(Some(lang), block) => {
			let Some(block) = block else {
				return Err(Error::new_spanned(&func.sig, "An empty block is required"));
			};

			if !block.stmts.is_empty() {
				return Err(Error::new_spanned(block, "This block must be empty"));
			}

			block.stmts.push(Stmt::Expr(
				match lang {
					Lang::GetContext => parse_quote! { __xx_internal_async_context }
				},
				None
			));
		}

		(None, None) => ()
	}

	let context_ident = quote! { __xx_internal_async_context };
	let context_type = quote! { ::xx_core::pointer::Ptr<::xx_core::coroutines::Context> };
	let args = [(context_ident.clone(), context_type.clone())];

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
				OpaqueClosureType::Custom(|rt: TokenStream, inlining: Inlining| {
					(
						quote_spanned! { rt.span() => ::xx_core::coroutines::Task<Output = #rt> },
						quote! { ::xx_core::coroutines::closure::OpaqueClosure::<_, _, { #inlining }> }
					)
				}),
				closure_type == ClosureType::OpaqueTrait
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

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum AsyncKind {
	Default,
	Explicit,
	TraitFn,
	TraitExt,
	Task
}

impl AsyncKind {
	#[must_use]
	const fn closure_type(self) -> ClosureType {
		match self {
			Self::Default => ClosureType::Opaque,
			Self::Explicit => ClosureType::Explicit,
			Self::TraitFn => ClosureType::None,
			Self::TraitExt => ClosureType::OpaqueTrait,
			Self::Task => ClosureType::None
		}
	}
}

fn parse_attrs(attrs: TokenStream) -> Result<AsyncKind> {
	let options = Punctuated::<Ident, Token![,]>::parse_terminated.parse2(attrs)?;
	let mut kind = AsyncKind::Default;

	for option in &options {
		if kind != AsyncKind::Default {
			return Err(Error::new_spanned(
				options,
				"Invalid combination of options"
			));
		}

		kind = match option.to_string().as_ref() {
			"explicit" => AsyncKind::Explicit,
			"traitfn" => AsyncKind::TraitFn,
			"traitext" => AsyncKind::TraitExt,
			"task" => AsyncKind::Task,
			_ => return Err(Error::new_spanned(option, "Unknown option"))
		};
	}

	Ok(kind)
}

fn try_transform(attrs: TokenStream, item: TokenStream) -> Result<TokenStream> {
	let async_kind = parse_attrs(attrs)?;
	let item = parse2::<Functions>(item)?;

	let transform_functions = |ty: ClosureType| {
		transform_functions(
			item.clone(),
			|func| transform_async(func, ty),
			|item| !matches!(item, Functions::Trait(_) | Functions::TraitFn(_))
		)
	};

	match async_kind {
		AsyncKind::Default => (),
		AsyncKind::TraitFn => return async_impl(item),
		_ => return transform_functions(async_kind.closure_type())
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
