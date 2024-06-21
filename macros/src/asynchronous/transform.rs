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
				tys.push(Type::Infer(TypeInfer {
					underscore_token: Default::default()
				}));
			}
		}
	}

	let (pats, tys) = (make_tuple_of_types(pats), make_tuple_of_types(tys));

	args.push(Pat::Type(parse_quote! { #pats: #tys }));
}

pub struct TransformAsync;

impl TransformAsync {
	fn transform_async(&mut self, inner: &mut ExprAsync) -> Expr {
		self.visit_expr_async_mut(inner);

		let (attrs, capture, block) = (&inner.attrs, &inner.capture, inner.block.clone());
		let context = Context::new();

		parse_quote_spanned! { inner.span() =>
			#(#attrs)*
			::xx_core::coroutines::internal::as_task(
				::xx_core::coroutines::closure::OpaqueClosure
					::new(
					#capture
					|#context| #block
				)
			)
		}
	}

	fn transform_await(&mut self, inner: &mut ExprAwait) -> Expr {
		self.visit_expr_await_mut(inner);

		let (attrs, base) = (&inner.attrs, inner.base.as_ref());
		let ident = Context::ident();
		let run = Ident::new("run", inner.await_token.span());

		parse_quote! {
			#(#attrs)*
			::xx_core::coroutines::Context::#run(#ident, #base)
		}
	}

	fn transform_closure(&mut self, closure: &mut ExprClosure) -> Expr {
		let asyncness = closure.asyncness.take();
		let body = closure.body.as_mut();

		#[allow(clippy::never_loop)]
		loop {
			if asyncness.is_some() {
				if !matches!(body, Expr::Block(_)) {
					*body = parse_quote! {{ #body }};
				}

				break;
			}

			if let Expr::Async(expr) = body {
				if expr.capture.is_none() {
					return error_on_tokens(
						expr.async_token,
						"Async closure is missing the `move` keyword"
					);
				}

				*body = Expr::Block(ExprBlock {
					attrs: expr.attrs.clone(),
					label: None,
					block: expr.block.clone()
				});

				break;
			}

			return Expr::Closure(closure.clone());
		}

		let context = Context::new();

		tuple_args(&mut closure.inputs);

		closure.inputs.push(Pat::Type(parse_quote! { #context }));

		self.visit_expr_mut(body);

		let attrs = &closure.attrs;

		parse_quote_spanned! { closure.span() =>
			#(#attrs)*
			::xx_core::coroutines::closure::OpaqueAsyncFn::new(#closure)
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
		*expr = match expr {
			Expr::Async(inner) => TransformAsync {}.transform_async(inner),
			Expr::Closure(inner) => TransformAsync {}.transform_closure(inner),
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
				::xx_core::coroutines::closure::OpaqueTask<F, Output>;
		}
	} else {
		let new = Ident::new("new", ident.span());
		let run = Ident::new("run", ident.span());

		quote! {
			struct XXInternalAsyncSupportWrap<F, Output>(F, std::marker::PhantomData<Output>);

			impl<F: ::std::ops::FnOnce(&::xx_core::coroutines::Context) -> Output, Output>
				XXInternalAsyncSupportWrap<F, Output> {
				#[inline(always)]
				pub const fn #new(func: F) -> Self {
					Self(func, std::marker::PhantomData)
				}
			}

			unsafe impl<F: ::std::ops::FnOnce(&::xx_core::coroutines::Context) -> Output, Output>
				::xx_core::coroutines::Task for XXInternalAsyncSupportWrap<F, Output> {
				type Output<'ctx> = Output;

				#(#attrs)*
				fn #run(self, context: &::xx_core::coroutines::Context) -> Output {
					self.0(context)
				}
			}
		}
	}
}

fn lending_task_impl(lt: &Lifetime, output: &Type) -> TokenStream {
	let mut ret = output.clone();

	ReplaceLifetime(lt).visit_type_mut(&mut ret);

	quote! {
		struct XXInternalAsyncSupportWrap<F>(F);

		impl<F: ::std::ops::FnOnce(&::xx_core::coroutines::Context) -> #ret>
			XXInternalAsyncSupportWrap<F> {
			#[inline(always)]
			pub const fn new(func: F) -> Self {
				Self(func)
			}
		}

		unsafe impl<F: ::std::ops::FnOnce(&::xx_core::coroutines::Context) -> #ret>
			::xx_core::coroutines::Task for XXInternalAsyncSupportWrap<F> {
			type Output<#lt> = #output;

			#[inline(always)]
			fn run(self, context: &::xx_core::coroutines::Context) -> #ret {
				self.0(context)
			}
		}
	}
}

pub fn transform_async(mut attrs: AttributeArgs, func: &mut Function<'_>) -> Result<()> {
	if func.sig.asyncness.take().is_none() {
		return if !func.is_root {
			Ok(())
		} else {
			let message = "The `async` keyword is missing from the function declaration";

			Err(Error::new_spanned(func.sig.fn_token, message))
		};
	}

	let closure_type = attrs.async_kind.0.closure_type();

	attrs.parse(func.attrs)?;

	let func_attrs = remove_attrs(func.attrs, &["inline", "must_use", "hot", "cold"]);

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

	let lifetime = if let Some(lifetime) = &attrs.context_lifetime {
		if lang.is_some() {
			return Err(Error::new(
				lifetime.span(),
				"Context lifetime forbidden in lang items"
			));
		}

		if !func.sig.generics.params.is_empty() {
			// TODO: temporary limitation
			return Err(Error::new_spanned(
				&func.sig.generics,
				"Generics not allowed here"
			));
		}

		if let Some(lt) = func
			.sig
			.generics
			.lifetimes()
			.find(|lt| lt.lifetime.ident == lifetime.ident)
		{
			return Err(Error::new_spanned(
				lt,
				"The context lifetime must not appear in the generics"
			));
		}

		if !matches!(closure_type, ClosureType::Standard | ClosureType::Trait) {
			return Err(Error::new(
				attrs.async_kind.1,
				"Unsupported closure type for this operation"
			));
		}

		lifetime.clone()
	} else {
		#[allow(clippy::collapsible_else_if)]
		if lang == Some(&Lang::GetContext) {
			let lt: Lifetime = parse_quote! { 'current };

			attrs.context_lifetime = Some(lt.clone());
			lt
		} else {
			parse_quote! { '__xx_internal_current_context }
		}
	};

	if let Some(block) = &mut func.block {
		TransformAsync {}.visit_block_mut(block);
	}

	let context = Context::new();
	let return_type = get_return_type(&func.sig.output);

	match closure_type {
		ClosureType::None => {
			func.sig.inputs.push(parse_quote! { #context });
		}

		_ => {
			make_opaque_closure(
				func,
				&[(context.ident, context.ty)],
				|rt| {
					let return_type = rt.to_token_stream();

					if let Some(lt) = &attrs.context_lifetime {
						ReplaceLifetime(lt).visit_type_mut(rt);
					}

					return_type
				},
				OpaqueClosureType::Custom(|rt: TokenStream| {
					(
						quote_spanned! { rt.span() =>
							for<#lifetime> ::xx_core::coroutines::Task<
								Output<#lifetime> = #rt
							>
						},
						quote! { XXInternalAsyncSupportWrap }
					)
				}),
				if closure_type != ClosureType::Trait && lang.is_none() {
					LifetimeAnnotations::Auto
				} else {
					LifetimeAnnotations::None
				}
			)?;
		}
	}

	if let Some(block) = &mut func.block {
		let task_impl = if let Some(lt) = &attrs.context_lifetime {
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
