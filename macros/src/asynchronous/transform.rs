use super::*;

pub struct Context {
	pub ident: TokenStream,
	pub ty: TokenStream
}

impl Context {
	pub fn new() -> Self {
		Self { ident: Self::ident(), ty: Self::ty() }
	}

	pub fn ident() -> TokenStream {
		quote_spanned! { Span::mixed_site() => context }
	}

	pub fn ty() -> TokenStream {
		quote! { &::xx_core::coroutines::Context }
	}
}

impl ToTokens for Context {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.ident.to_tokens(tokens);

		quote! { : }.to_tokens(tokens);

		self.ty.to_tokens(tokens);
	}
}

fn tuple_args(args: &mut Punctuated<Pat, Token![,]>) {
	let (mut pats, mut tys) = (Vec::new(), Vec::new());

	for input in take(args) {
		match input {
			Pat::Type(ty) => {
				pats.push(*ty.pat);
				tys.push(*ty.ty);
			}

			_ => {
				pats.push(input);
				tys.push(parse_quote! { _ });
			}
		}
	}

	let (pats, tys) = (join_tuple(pats), join_tuple(tys));

	args.push(Pat::Type(parse_quote! { #pats: #tys }));
}

#[derive(Default)]
pub struct TransformAsync {
	in_async: bool,
	has_await: bool
}

impl TransformAsync {
	fn transform_async(&mut self, inner: &mut ExprAsync) -> Expr {
		self.in_async = true;
		self.visit_expr_async_mut(inner);
		self.in_async = false;

		let (attrs, capture, block) = (&inner.attrs, &inner.capture, &inner.block);
		let context = Context::new();

		parse_quote_spanned! { inner.async_token.span() =>
			#(#attrs)*
			::xx_core::coroutines::internal::as_task(
				::xx_core::coroutines::internal::OpaqueClosure::new(
					#capture
					|#context| #block
				)
			)
		}
	}

	fn transform_await(&mut self, inner: &mut ExprAwait) -> Expr {
		self.visit_expr_await_mut(inner);

		if !self.in_async {
			self.has_await = true;
		}

		let (attrs, base) = (&inner.attrs, &inner.base);
		let ident = Context::ident();

		parse_quote_spanned! { inner.await_token.span() =>
			#(#attrs)*
			::xx_core::coroutines::internal::unsafe_stub_do_not_use(#ident, #base)
		}
	}

	fn transform_closure(&mut self, closure: &mut ExprClosure) -> Expr {
		let body = closure.body.as_mut();
		let span;

		#[allow(clippy::never_loop)]
		loop {
			if let Some(asyncness) = closure.asyncness.take() {
				span = asyncness.span();

				if !matches!(body, Expr::Block(_)) {
					*body = parse_quote! {{ #body }};
				}

				break;
			}

			if let Expr::Async(expr) = body {
				if expr.capture.is_some() {
					span = expr.async_token.span();

					*body = Expr::Block(ExprBlock {
						attrs: expr.attrs.clone(),
						label: None,
						block: expr.block.clone()
					});

					break;
				}
			}

			TransformItems.visit_expr_closure_mut(closure);

			return Expr::Closure(closure.clone());
		}

		let context = Context::new();

		tuple_args(&mut closure.inputs);

		closure.inputs.push(Pat::Type(parse_quote! { #context }));

		self.in_async = true;
		self.visit_expr_mut(body);
		self.in_async = false;

		let attrs = &closure.attrs;

		parse_quote_spanned! { span =>
			#(#attrs)*
			::xx_core::coroutines::internal::OpaqueAsyncFn::new(#closure)
		}
	}
}

impl VisitMut for TransformAsync {
	fn visit_item_mut(&mut self, _: &mut Item) {}

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

pub struct TransformItems;

impl VisitMut for TransformItems {
	fn visit_item_mut(&mut self, _: &mut Item) {}

	fn visit_expr_mut(&mut self, expr: &mut Expr) {
		let mut visit = TransformAsync::default();

		*expr = match expr {
			Expr::Async(inner) => visit.transform_async(inner),
			Expr::Closure(inner) => visit.transform_closure(inner),
			_ => return visit_expr_mut(self, expr)
		};
	}

	fn visit_macro_mut(&mut self, mac: &mut Macro) {
		visit_macro_body(self, mac);
	}
}

pub struct ReplaceLifetime<'a>(pub &'a Lifetime);

impl VisitMut for ReplaceLifetime<'_> {
	fn visit_lifetime_mut(&mut self, lt: &mut Lifetime) {
		if lt.ident == self.0.ident {
			lt.ident = Ident::new("_", lt.ident.span());
		}
	}
}

fn task_impl(attrs: &[Attribute], ident: &Ident) -> TokenStream {
	if attrs.is_empty() {
		quote! {
			type XXInternalAsyncSupportWrap<F, Output> =
				::xx_core::coroutines::internal::OpaqueTask<F, Output>;
		}
	} else {
		let new = Ident::new("new", ident.span());
		let run = Ident::new("run", ident.span());

		quote! {
			struct XXInternalAsyncSupportWrap<F, Output>(F, ::std::marker::PhantomData<Output>);

			const _: () = {
				use ::std::ops::FnOnce;
				use ::std::marker::PhantomData;
				use ::xx_core::coroutines::{Context, Task};

				impl<F: FnOnce(&Context) -> Output, Output> XXInternalAsyncSupportWrap<F, Output> {
					#[inline(always)]
					pub const fn #new(func: F) -> Self {
						Self(func, PhantomData)
					}
				}

				impl<F: FnOnce(&Context) -> Output, Output> Task
					for XXInternalAsyncSupportWrap<F, Output> {
					type Output<'ctx> = Output;

					#(#attrs)*
					unsafe fn #run(self, context: &Context) -> Output {
						self.0(context)
					}
				}
			};
		}
	}
}

fn lending_task_impl(lt: &Lifetime, output: &Type) -> TokenStream {
	let mut ret = output.clone();
	let context = Context::ty();

	ReplaceLifetime(lt).visit_type_mut(&mut ret);

	quote! {
		struct XXInternalAsyncSupportWrap<F>(F);

		const _: () = {
			use ::std::ops::FnOnce;
			use ::std::marker::PhantomData;
			use ::xx_core::coroutines::{Context, Task};

			impl<F: FnOnce(&Context) -> #ret> XXInternalAsyncSupportWrap<F> {
				#[inline(always)]
				pub const fn new(func: F) -> Self {
					Self(func)
				}
			}

			impl<F: FnOnce(&Context) -> #ret> Task for XXInternalAsyncSupportWrap<F> {
				type Output<#lt> = #output;

				#[inline(always)]
				unsafe fn run(self, context: #context) -> #ret {
					self.0(context)
				}
			}
		};
	}
}

#[derive(Default)]
struct AddLifetimes(Vec<Lifetime>);

impl AddLifetimes {
	fn next_lifetime(&mut self, span: Span) -> Lifetime {
		let lifetime = Lifetime::new(&format!("'__xx_hrlt_{}", self.0.len() + 1), span);

		self.0.push(lifetime.clone());

		lifetime
	}
}

impl VisitMut for AddLifetimes {
	fn visit_type_reference_mut(&mut self, reference: &mut TypeReference) {
		if reference.lifetime.is_some() {
			return;
		}

		if !matches!(reference.elem.as_ref(), Type::ImplTrait(_)) {
			visit_type_reference_mut(self, reference);
		}

		reference.lifetime = Some(self.next_lifetime(reference.span()));
	}

	fn visit_lifetime_mut(&mut self, lifetime: &mut Lifetime) {
		if lifetime.ident == "_" {
			*lifetime = self.next_lifetime(lifetime.span());
		} else {
			self.0.push(lifetime.clone());
		}
	}
}

#[allow(clippy::missing_panics_doc)]
fn modify_bounds(bounds: &mut Punctuated<TypeParamBound, Token![+]>) -> Result<()> {
	let path_segments = ["xx_core", "coroutines", "ops"];
	let async_fns = ["AsyncFnOnce", "AsyncFnMut", "AsyncFn"];

	for bound in bounds {
		let TypeParamBound::Trait(TraitBound { lifetimes, path, .. }) = bound else {
			continue;
		};

		let Some(last) = path.segments.last() else {
			continue;
		};

		if !async_fns.contains(&last.ident.to_string().as_ref()) ||
			!matches!(last.arguments, PathArguments::Parenthesized(_))
		{
			continue;
		}

		if path.segments.len() != 1 {
			let start_index = if path.leading_colon.is_some() {
				0
			} else {
				let Some(pos) = path_segments
					.iter()
					.position(|name| path.segments[0].ident == name)
				else {
					continue;
				};

				pos
			};

			if path_segments.len() - start_index != path.segments.len() - 1 {
				continue;
			}

			if path_segments[start_index..]
				.iter()
				.zip(&path.segments)
				.any(|(expect, segment)| segment.ident != expect || !segment.arguments.is_none())
			{
				continue;
			}
		}

		let last = path.segments.last_mut().unwrap();
		let PathArguments::Parenthesized(args) = &mut last.arguments else {
			unreachable!()
		};

		let mut op = AddLifetimes::default();
		let mut tys = Vec::new();

		for mut ty in take(&mut args.inputs) {
			op.visit_type_mut(&mut ty);
			tys.push(ty);
		}

		let inputs = join_tuple(tys);
		let output = get_return_type(&args.output);

		last.arguments = PathArguments::AngleBracketed(parse_quote! {
			<#inputs, Output = #output>
		});

		for lt in op.0 {
			let bound = lifetimes.get_or_insert_with(|| BoundLifetimes {
				for_token: Default::default(),
				lt_token: Default::default(),
				lifetimes: Default::default(),
				gt_token: Default::default()
			});

			bound.lifetimes.push(parse_quote! { #lt });
		}
	}

	Ok(())
}

fn modify_traits(func: &mut Function<'_>) -> Result<()> {
	for param in &mut func.sig.generics.params {
		let GenericParam::Type(ty) = param else {
			continue;
		};

		modify_bounds(&mut ty.bounds)?;
	}

	if let Some(clause) = &mut func.sig.generics.where_clause {
		for predicate in &mut clause.predicates {
			let WherePredicate::Type(ty) = predicate else {
				continue;
			};

			modify_bounds(&mut ty.bounds)?;
		}
	}

	for input in &mut func.sig.inputs {
		let FnArg::Typed(pat) = input else { continue };
		let Type::ImplTrait(imp) = pat.ty.as_mut() else {
			continue;
		};

		modify_bounds(&mut imp.bounds)?;
	}

	if let ReturnType::Type(_, ty) = &mut func.sig.output {
		if let Type::ImplTrait(imp) = ty.as_mut() {
			modify_bounds(&mut imp.bounds)?;
		};
	}

	Ok(())
}

pub fn transform_items(func: &mut Function<'_>) -> Result<()> {
	if let Some(block) = &mut func.block {
		TransformItems.visit_block_mut(block);
	}

	modify_traits(func)
}

pub fn transform_async(mut attrs: AttributeArgs, func: &mut Function<'_>) -> Result<()> {
	let Some(asyncness) = func.sig.asyncness.take() else {
		return if !func.is_root {
			transform_items(func)
		} else {
			let message = "The `async` keyword is missing from the function declaration";

			Err(Error::new_spanned(func.sig.fn_token, message))
		};
	};

	modify_traits(func)?;

	let closure_type = attrs.async_kind.0.closure_type();

	attrs.parse_additional(func.attrs)?;

	let func_attrs = remove_attrs(func.attrs, &["inline", "must_use", "cold"]);

	let lang = match &attrs.language {
		Some((lang, span)) => {
			let Some(block) = &mut func.block else {
				#[allow(clippy::needless_borrows_for_generic_args)]
				return Err(Error::new_spanned(&func.sig, "An empty block is required"));
			};

			if !block.stmts.is_empty() {
				return Err(Error::new_spanned(block, "This block must be empty"));
			}

			block.stmts.push(parse_quote_spanned! { *span =>
				#[allow(unused_imports)]
				use ::xx_core::coroutines::lang;
			});

			block.stmts.push(Stmt::Expr(
				match lang {
					Lang::GetContext => {
						let context = Context::ident();

						parse_quote! { #context }
					}
					_ => unreachable!()
				},
				None
			));

			Some(lang)
		}

		None => None
	};

	let mut context_lifetime = None;

	if let Some(GenericParam::Lifetime(param)) = func.sig.generics.params.first_mut() {
		context_lifetime = remove_attr_path(&mut param.attrs, "cx").map(|_| param.lifetime.clone());
	}

	let lifetime = if let Some(lifetime) = &context_lifetime {
		if func.sig.generics.params.len() != 1 {
			let msg = "Generics not allowed here";

			return Err(Error::new_spanned(&func.sig.generics, msg));
		}

		func.sig.generics.params.clear();

		if lang.is_some() {
			let msg = "Context lifetime forbidden in lang items";

			return Err(Error::new_spanned(lifetime, msg));
		}

		if !matches!(closure_type, ClosureType::Standard | ClosureType::Trait) {
			let msg = "Unsupported closure type for this operation";

			return Err(Error::new(attrs.async_kind.1, msg));
		}

		lifetime.clone()
	} else if lang == Some(&Lang::GetContext) {
		let lt: Lifetime = parse_quote! { 'current };

		context_lifetime = Some(lt.clone());
		lt
	} else {
		parse_quote! { '__xx_icurctx }
	};

	if let Some(block) = &mut func.block {
		let mut visit = TransformAsync::default();

		visit.visit_block_mut(block);

		if !visit.has_await && lang.is_none() && attrs.async_kind.0 != AsyncKind::TraitFn {
			let warning = parse_quote_spanned! { asyncness.span() =>
				const _: () = { async fn warning() {} };
			};

			block.stmts.insert(0, warning);
		}
	}

	let context = Context::new();
	let return_type = get_return_type(&func.sig.output);

	if closure_type == ClosureType::None {
		func.sig.inputs.push(parse_quote! { #context });
	} else {
		let map_return_type = |rt: &mut Type| {
			let return_type = rt.to_token_stream();

			if let Some(lt) = &context_lifetime {
				ReplaceLifetime(lt).visit_type_mut(rt);
			}

			return_type
		};

		let impl_type = |rt: TokenStream| {
			(
				quote_spanned! { rt.span() =>
					for<#lifetime> ::xx_core::coroutines::Task<Output<#lifetime> = #rt>
				},
				quote! { XXInternalAsyncSupportWrap }
			)
		};

		let annotations = if closure_type != ClosureType::Trait && lang.is_none() {
			LifetimeAnnotations::Auto
		} else {
			LifetimeAnnotations::None
		};

		make_opaque_closure(
			func,
			&[(context.ident, context.ty)],
			map_return_type,
			OpaqueClosureType::Custom(impl_type),
			annotations
		)?;
	}

	if let Some(block) = &mut func.block {
		let task_impl = if let Some(lt) = &context_lifetime {
			lending_task_impl(lt, &return_type)
		} else {
			task_impl(&func_attrs, &func.sig.ident)
		};

		let stmts = &block.stmts;

		**block = parse_quote! {{
			#task_impl
			#(#stmts)*
		}};
	}

	Ok(())
}
