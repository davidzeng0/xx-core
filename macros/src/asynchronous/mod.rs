use std::mem::take;

use syn::parse::discouraged::Speculative;

use super::*;

pub mod branch;
mod traits;

use traits::*;

struct Context(TokenStream, TokenStream);

impl Context {
	fn new() -> Self {
		Self(
			quote_spanned! { Span::mixed_site() => context },
			quote! { &::xx_core::coroutines::Context }
		)
	}
}

impl ToTokens for Context {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.0.to_tokens(tokens);

		quote! { : }.to_tokens(tokens);

		self.1.to_tokens(tokens);
	}
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ClosureType {
	None,
	Standard,
	Trait
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum AsyncKind {
	Default,
	TraitFn,
	TraitExt,
	Task,
	Block
}

impl AsyncKind {
	#[must_use]
	const fn closure_type(self) -> ClosureType {
		match self {
			Self::Default => ClosureType::Standard,
			Self::TraitFn => ClosureType::None,
			Self::TraitExt => ClosureType::Trait,
			Self::Task => ClosureType::None,
			Self::Block => ClosureType::None
		}
	}

	fn from_str(str: &str) -> Option<Self> {
		Some(match str {
			"traitfn" => Self::TraitFn,
			"traitext" => Self::TraitExt,
			"task" => Self::Task,
			"block" => Self::Block,
			_ => return None
		})
	}
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Lang {
	GetContext,
	TaskClosure,
	AsyncClosure
}

#[derive(Clone)]
struct AttributeArgs {
	async_kind: (AsyncKind, Span),
	language: Option<(Lang, Span)>,
	context_lifetime: Option<Lifetime>
}

impl AttributeArgs {
	const fn new(async_kind: AsyncKind, span: Span) -> Self {
		Self {
			async_kind: (async_kind, span),
			language: None,
			context_lifetime: None
		}
	}

	fn parse(&mut self, attrs: &mut Vec<Attribute>) -> Result<()> {
		self.language = get_lang(attrs)?;
		self.context_lifetime = get_context_lifetime(attrs)?;

		Ok(())
	}
}

#[derive(Clone)]
enum AsyncItem {
	Fn(ImplItemFn),
	TraitFn(TraitItemFn),
	Trait(ItemTrait),
	Impl(ItemImpl),
	Struct(ItemStruct)
}

impl Parse for AsyncItem {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let fork = input.fork();

		if let Ok(item) = ItemStruct::parse(&fork) {
			input.advance_to(&fork);

			return Ok(Self::Struct(item));
		}

		Ok(match Functions::parse(input)? {
			Functions::Fn(item) => Self::Fn(item),
			Functions::TraitFn(item) => Self::TraitFn(item),
			Functions::Trait(item) => Self::Trait(item),
			Functions::Impl(item) => Self::Impl(item)
		})
	}
}

struct TransformAsync;

impl TransformAsync {
	fn transform_async(&mut self, inner: &mut ExprAsync) -> Expr {
		self.visit_expr_async_mut(inner);

		let (attrs, capture, block) = (&inner.attrs, &inner.capture, inner.block.clone());
		let context = Context::new();

		parse_quote_spanned! { inner.span() =>
			#(#attrs)*
			::xx_core::coroutines::closure::OpaqueClosure
				::new(
				#capture
				|#context| #block
			)
		}
	}

	fn transform_await(&mut self, inner: &mut ExprAwait) -> Expr {
		self.visit_expr_await_mut(inner);

		let (attrs, base) = (&inner.attrs, inner.base.as_ref());
		let ident = Context::new().0;

		parse_quote_spanned! { inner.await_token.span() =>
			#(#attrs)*
			::xx_core::coroutines::Context::run(#ident, #base)
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
					return error_to_tokens(Error::new_spanned(
						expr.async_token,
						"Async closure is missing the `move` keyword"
					));
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

		parse_quote_spanned! { closure.span() =>
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

struct ReplaceLifetime<'a>(&'a Lifetime);

impl VisitMut for ReplaceLifetime<'_> {
	fn visit_lifetime_mut(&mut self, lt: &mut Lifetime) {
		if lt.ident == self.0.ident {
			lt.ident = format_ident!("_", span = lt.ident.span());
		}
	}
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
				"task_closure" => Lang::TaskClosure,
				"async_closure" => Lang::AsyncClosure,
				_ => return Err(Error::new_spanned(str, "Unknown lang item"))
			},
			attr.span()
		));
	}

	Ok(lang)
}

fn get_context_lifetime(attrs: &mut Vec<Attribute>) -> Result<Option<Lifetime>> {
	let mut lifetime = None;

	if let Some(attr) = remove_attr_list(attrs, "context") {
		let Ok(lt) = parse2(attr.tokens.clone()) else {
			return Err(Error::new_spanned(attr.tokens, "Expected a lifetime"));
		};

		lifetime = Some(lt);
	}

	Ok(lifetime)
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

fn remove_attrs(attrs: &mut Vec<Attribute>, targets: &[&str]) -> Vec<Attribute> {
	let mut removed = Vec::new();

	for target in targets {
		while let Some(attr) = remove_attr_kind(attrs, target, |_| true) {
			removed.push(attr);
		}
	}

	removed
}

fn task_impl(attrs: &[Attribute], ident: &Ident) -> TokenStream {
	let run = quote_spanned! { ident.span() =>
		#(#attrs)*
		fn run(self, context: &::xx_core::coroutines::Context) -> Output {
			self.0(context)
		}
	};

	quote! {
		#[allow(non_camel_case_types)]
		struct __xx_internal_async_support_wrap<F, Output>(F, std::marker::PhantomData<Output>);

		impl<F: ::std::ops::FnOnce(&::xx_core::coroutines::Context) -> Output, Output>
			__xx_internal_async_support_wrap<F, Output> {
			#[inline(always)]
			pub const fn new(func: F) -> Self {
				Self(func, std::marker::PhantomData)
			}
		}

		unsafe impl<F: ::std::ops::FnOnce(&::xx_core::coroutines::Context) -> Output, Output>
			::xx_core::coroutines::Task for __xx_internal_async_support_wrap<F, Output> {
			type Output<'a> = Output;

			#run
		}
	}
}

fn lending_task_impl(lt: &Lifetime, output: &Type) -> TokenStream {
	let mut ret = output.clone();

	ReplaceLifetime(lt).visit_type_mut(&mut ret);

	quote! {
		#[allow(non_camel_case_types)]
		struct __xx_internal_async_support_wrap<F>(F);

		impl<F: ::std::ops::FnOnce(&::xx_core::coroutines::Context) -> #ret>
			__xx_internal_async_support_wrap<F> {
			#[inline(always)]
			pub const fn new(func: F) -> Self {
				Self(func)
			}
		}

		unsafe impl<F: ::std::ops::FnOnce(&::xx_core::coroutines::Context) -> #ret>
			::xx_core::coroutines::Task for __xx_internal_async_support_wrap<F> {
			type Output<#lt> = #output;

			#[inline(always)]
			fn run(self, context: &::xx_core::coroutines::Context) -> #ret {
				self.0(context)
			}
		}
	}
}

fn transform_async(mut attrs: AttributeArgs, func: &mut Function<'_>) -> Result<()> {
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

	if closure_type != ClosureType::None {
		func.attrs.push(parse_quote! {
			#[must_use = "Task does nothing until you `.await` it"]
		});
	}

	let lang = match &attrs.language {
		Some((lang, span)) => {
			let Some(block) = &mut func.block else {
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
						let context = Context::new().0;

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
				&[(context.0, context.1)],
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
						quote! { __xx_internal_async_support_wrap }
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

fn parse_attrs(attrs: TokenStream) -> Result<AttributeArgs> {
	let mut parsed = AttributeArgs::new(AsyncKind::Default, attrs.span());

	let options = Punctuated::<Ident, Token![,]>::parse_terminated.parse2(attrs)?;

	for option in &options {
		if parsed.async_kind.0 != AsyncKind::Default {
			let message = "Invalid combination of options";

			return Err(Error::new_spanned(options, message));
		}

		let kind = AsyncKind::from_str(&option.to_string())
			.ok_or_else(|| Error::new_spanned(option, "Unknown option"))?;
		parsed.async_kind = (kind, option.span());
	}

	Ok(parsed)
}

fn task_closure_impl(use_lang: TokenStream, item: ItemStruct) -> TokenStream {
	let ident = &item.ident;
	let context = Context::new();
	let context_ident = &context.0;

	quote! {
		#item

		impl<F: FnOnce(&Context) -> Output, Output> #ident<F, Output> {
			#[inline(always)]
			#[must_use = "Task does nothing until you `.await` it"]
			pub const fn new(func: F) -> Self {
				#use_lang

				Self(func, PhantomData)
			}
		}

		unsafe impl<F: FnOnce(&Context) -> Output, Output> Task for #ident<F, Output> {
			type Output<'a> = Output;

			#[inline(always)]
			fn run(self, #context) -> Output {
				#use_lang

				self.0(#context_ident)
			}
		}
	}
}

fn async_closure_impl(use_lang: TokenStream, item: ItemStruct) -> TokenStream {
	let ident = &item.ident;

	quote! {
		#item

		impl<F, const T: usize> #ident<F, T> {
			#[inline(always)]
			pub const fn new(func: F) -> Self {
				#use_lang

				Self(func)
			}
		}

		impl<F: FnOnce(Args, &Context) -> Output, Args, Output> AsyncFnOnce<Args> for #ident<F, 0> {
			type Output = Output;

			#[asynchronous(traitext)]
			#[inline(always)]
			async fn call_once(self, args: Args) -> Output {
				#use_lang

				self.0(args, unsafe { get_context().await })
			}
		}

		impl<F: FnMut(Args, &Context) -> Output, Args, Output> AsyncFnMut<Args> for #ident<F, 1> {
			type Output = Output;

			#[asynchronous(traitext)]
			#[inline(always)]
			async fn call_mut(&mut self, args: Args) -> Output {
				#use_lang

				self.0(args, unsafe { get_context().await })
			}
		}

		impl<F: Fn(Args, &Context) -> Output, Args, Output> AsyncFn<Args> for #ident<F, 2> {
			type Output = Output;

			#[asynchronous(traitext)]
			#[inline(always)]
			async fn call(&self, args: Args) -> Output {
				#use_lang

				self.0(args, unsafe { get_context().await })
			}
		}
	}
}

fn language_impl(attrs: AttributeArgs, item: AsyncItem) -> Result<TokenStream> {
	let (lang, span) = attrs.language.unwrap();
	let use_lang = quote_spanned! { span =>
		#[allow(unused_imports)]
		use ::xx_core::coroutines::lang;
	};

	match (lang, item) {
		(Lang::TaskClosure, AsyncItem::Struct(item)) => Ok(task_closure_impl(use_lang, item)),
		(Lang::AsyncClosure, AsyncItem::Struct(item)) => Ok(async_closure_impl(use_lang, item)),
		_ => Err(Error::new(span, "Invalid language item"))
	}
}

fn async_block(item: Functions) -> Result<TokenStream> {
	let Functions::Fn(func) = item else {
		return Err(Error::new_spanned(item, "Expected a function"));
	};

	let block = func.block;
	let mut expr = Expr::Async(parse_quote! { async move #block });

	TransformAsync {}.visit_expr_mut(&mut expr);

	Ok(expr.to_token_stream())
}

fn try_transform(mut attrs: AttributeArgs, item: TokenStream) -> Result<TokenStream> {
	let mut item = parse2::<AsyncItem>(item)?;

	if attrs.async_kind.0 == AsyncKind::Task {
		if let AsyncItem::Impl(imp) = &mut item {
			/* hides the context pointer from the user, so this is safe */
			imp.unsafety = Some(Default::default());
		}
	}

	match &mut item {
		AsyncItem::Struct(item) => {
			attrs.parse(&mut item.attrs)?;
		}

		AsyncItem::Trait(item) => {
			attrs.parse(&mut item.attrs)?;
		}

		AsyncItem::Impl(imp) => {
			attrs.parse(&mut imp.attrs)?;
		}

		_ => ()
	}

	if let Some(lt) = &attrs.context_lifetime {
		return Err(Error::new_spanned(lt, "Context lifetime not allowed here"));
	}

	if attrs.language.is_some() {
		return language_impl(attrs, item);
	}

	let item = match item {
		AsyncItem::Fn(item) => Functions::Fn(item),
		AsyncItem::TraitFn(item) => Functions::TraitFn(item),
		AsyncItem::Trait(item) => Functions::Trait(item),
		AsyncItem::Impl(item) => Functions::Impl(item),
		AsyncItem::Struct(item) => return Err(Error::new_spanned(item, "Unexpected declaration"))
	};

	let transform_functions = |attrs: AttributeArgs| {
		item.clone().transform_all(
			|func| transform_async(attrs.clone(), func),
			|item| {
				attrs.async_kind.0 == AsyncKind::Task ||
					!matches!(item, Functions::Trait(_) | Functions::TraitFn(_))
			}
		)
	};

	match attrs.async_kind.0 {
		AsyncKind::Default => (),
		AsyncKind::TraitFn => return async_impl(attrs, item),
		AsyncKind::Block => return async_block(item),
		_ => return transform_functions(attrs)
	}

	match &item {
		Functions::Trait(item) => async_trait(attrs, item.clone()),
		Functions::Impl(imp) if imp.trait_.is_some() => async_impl(attrs, item.clone()),
		Functions::Fn(_) | Functions::Impl(_) => transform_functions(attrs),
		Functions::TraitFn(_) => {
			let message = "Trait functions must specify `#[asynchronous(traitfn)]`";

			Err(Error::new(Span::call_site(), message))
		}
	}
}

pub fn asynchronous(attrs: TokenStream, item: TokenStream) -> TokenStream {
	try_expand(|| {
		let attrs = parse_attrs(attrs)?;

		try_transform(attrs, item)
	})
}
