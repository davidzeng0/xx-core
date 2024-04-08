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
	let stmt = parse_quote! {
		let __xx_internal_async_context = unsafe {
			::xx_core::pointer::Ptr::<
				::xx_core::coroutines::Context
			>::as_ref(__xx_internal_async_context)
		};
	};

	block.stmts.insert(index, stmt);
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
				tys.push(Type::Infer(TypeInfer {
					underscore_token: Default::default()
				}));
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
					_ => block = parse_quote! {{ #body }}
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

		tuple_args(&mut closure.inputs);

		closure.inputs.push(Pat::Type(parse_quote! {
			__xx_internal_async_context: ::xx_core::pointer::Ptr<
				::xx_core::coroutines::Context
			>
		}));

		transform_block(&mut block.block);

		*body = Expr::Block(block);

		self.visit_expr_mut(body);

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

fn get_lang(attrs: &mut Vec<Attribute>) -> Result<Option<(Lang, Span)>> {
	let mut lang = None;

	if let Some(attr) = remove_attr_name_value(attrs, "lang") {
		let Expr::Lit(ExprLit { lit: Lit::Str(str), .. }) = &attr.value else {
			return Err(Error::new_spanned(attr.value, "Expected a str"));
		};

		lang = Some((
			match str.value().as_ref() {
				"get_context" => Lang::GetContext,
				_ => return Err(Error::new_spanned(str, "Unknown lang item"))
			},
			attr.span()
		));
	}

	Ok(lang)
}

fn remove_attrs(attrs: &mut Vec<Attribute>, targets: &[&str]) -> Vec<Attribute> {
	let mut removed = Vec::new();

	for target in targets {
		while let Some(attr) = remove_attr_kind(attrs, target, |_| true) {
			removed.push(attr);
		}
	}

	removed
}

fn transform_async(func: &mut Function<'_>, closure_type: ClosureType) -> Result<()> {
	if func.sig.asyncness.take().is_none() {
		return if !func.is_root {
			Ok(())
		} else {
			let message = "The `async` keyword is missing from the function declaration";

			Err(Error::new_spanned(func.sig.fn_token, message))
		};
	}

	let attrs = remove_attrs(func.attrs, &["inline", "must_use"]);

	if closure_type != ClosureType::None {
		func.attrs
			.push(parse_quote!( #[must_use = "Task does nothing until you `.await` it"] ));
	}

	match (get_lang(func.attrs)?, &mut func.block) {
		(None, Some(block)) => {
			transform_block(block);

			TransformAsync {}.visit_block_mut(block);
		}

		(Some((lang, span)), block) => {
			let Some(block) = block else {
				return Err(Error::new_spanned(&func.sig, "An empty block is required"));
			};

			if !block.stmts.is_empty() {
				return Err(Error::new_spanned(block, "This block must be empty"));
			}

			block.stmts.push(parse_quote_spanned! { span =>
				#[allow(unused_imports)]
				use ::xx_core::coroutines::lang;
			});

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
	let args = [(context_ident, context_type)];

	match closure_type {
		ClosureType::None => {
			let (ident, ty) = &args[0];

			func.sig.inputs.push(parse_quote! { #ident: #ty });
		}

		ClosureType::Opaque | ClosureType::OpaqueTrait => {
			make_opaque_closure(
				func,
				&args,
				|rt| rt,
				OpaqueClosureType::Custom(|rt: TokenStream| {
					(
						quote_spanned! { rt.span() => ::xx_core::coroutines::Task<Output = #rt> },
						quote! { __xx_internal_async_support::Wrap }
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

	if let Some(block) = &mut func.block {
		block.stmts.insert(
			0,
			parse_quote! {
				mod __xx_internal_async_support {
					use std::marker::PhantomData;
					use xx_core::{coroutines::*, pointer::*};

					pub struct Wrap<F, Args, Output>(F, PhantomData<(Args, Output)>);

					impl<F: FnOnce(Args) -> Output, Args, Output> Wrap<F, Args, Output> {
						#[inline(always)]
						pub const fn new(func: F) -> Self {
							Self(func, PhantomData)
						}
					}

					impl<F: FnOnce(Ptr<Context>) -> Output, Output> Task for Wrap<F, Ptr<Context>, Output> {
						type Output = Output;

						#(#attrs)*
						fn run(self, context: Ptr<Context>) -> Output {
							self.0(context)
						}
					}
				}
			}
		);
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

	fn from_str(str: &str) -> Option<Self> {
		Some(match str {
			"explicit" => Self::Explicit,
			"traitfn" => Self::TraitFn,
			"traitext" => Self::TraitExt,
			"task" => Self::Task,
			_ => return None
		})
	}
}

fn parse_attrs(attrs: TokenStream) -> Result<AsyncKind> {
	let options = Punctuated::<Ident, Token![,]>::parse_terminated.parse2(attrs)?;
	let mut kind = AsyncKind::Default;

	for option in &options {
		if kind != AsyncKind::Default {
			let message = "Invalid combination of options";

			return Err(Error::new_spanned(options, message));
		}

		kind = AsyncKind::from_str(&option.to_string())
			.ok_or_else(|| Error::new_spanned(option, "Unknown option"))?;
	}

	Ok(kind)
}

fn try_transform(attrs: TokenStream, item: TokenStream) -> Result<TokenStream> {
	let async_kind = parse_attrs(attrs)?;
	let item = parse2::<Functions>(item)?;

	let transform_functions = |ty: ClosureType| {
		item.clone().transform_all(
			|func| transform_async(func, ty),
			|item| {
				ty == ClosureType::None ||
					!matches!(item, Functions::Trait(_) | Functions::TraitFn(_))
			}
		)
	};

	match async_kind {
		AsyncKind::Default => (),
		AsyncKind::TraitFn => return async_impl(item),
		kind => return transform_functions(kind.closure_type())
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
