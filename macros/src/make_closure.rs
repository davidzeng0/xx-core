use super::*;

fn closure_lifetime() -> TokenStream {
	quote! { '__xx_internal_closure_lifetime }
}

fn closure_lifetime_parsed<R>() -> R
where
	R: Parse
{
	parse2(closure_lifetime()).unwrap()
}

const SELF_IDENT: &str = "__xx_internal_self";

pub struct ReplaceSelf;

impl VisitMut for ReplaceSelf {
	fn visit_fn_arg_mut(&mut self, arg: &mut FnArg) {
		visit_fn_arg_mut(self, arg);

		let FnArg::Receiver(rec) = arg else { return };

		let (attrs, mut mutability, ty) = (&rec.attrs, rec.mutability, &rec.ty);
		let ident = format_ident!("{}", SELF_IDENT);

		if rec.reference.is_some() {
			mutability = None;
		}

		*arg = parse_quote! {
			#(#attrs)*
			#mutability #ident: #ty
		}
	}

	fn visit_ident_mut(&mut self, ident: &mut Ident) {
		visit_ident_mut(self, ident);

		if ident == "self" {
			*ident = format_ident!("{}", SELF_IDENT, span = ident.span());
		}
	}

	fn visit_macro_mut(&mut self, mac: &mut Macro) {
		visit_macro_punctuated_exprs(self, mac);
	}
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LifetimeAnnotations {
	Auto,
	Closure,
	None
}

struct AddLifetime {
	annotations: LifetimeAnnotations,
	explicit_lifetimes: Vec<Lifetime>,
	added_lifetimes: Vec<Lifetime>
}

impl AddLifetime {
	fn new(annotations: LifetimeAnnotations) -> Self {
		Self {
			annotations,
			explicit_lifetimes: Vec::new(),
			added_lifetimes: Vec::new()
		}
	}
}

impl AddLifetime {
	fn next_lifetime(&mut self, span: Span) -> Lifetime {
		if self.annotations == LifetimeAnnotations::Closure {
			return closure_lifetime_parsed();
		}

		let lifetime = Lifetime::new(
			&format!("{}_{}", closure_lifetime(), self.added_lifetimes.len() + 1),
			span
		);

		self.added_lifetimes.push(lifetime.clone());

		lifetime
	}
}

impl VisitMut for AddLifetime {
	fn visit_type_reference_mut(&mut self, reference: &mut TypeReference) {
		if let Some(lifetime) = &reference.lifetime {
			self.explicit_lifetimes.push(lifetime.clone());

			return;
		}

		if !matches!(reference.elem.as_ref(), Type::ImplTrait(_)) {
			visit_type_reference_mut(self, reference);
		}

		reference.lifetime = Some(self.next_lifetime(reference.span()));
	}

	fn visit_lifetime_mut(&mut self, lifetime: &mut Lifetime) {
		visit_lifetime_mut(self, lifetime);

		if lifetime.ident == "_" {
			*lifetime = self.next_lifetime(lifetime.span());
		} else {
			self.explicit_lifetimes.push(lifetime.clone());
		}
	}

	fn visit_receiver_mut(&mut self, rec: &mut Receiver) {
		if let Some(reference) = &mut rec.reference {
			if let Some(lifetime) = &reference.1 {
				self.explicit_lifetimes.push(lifetime.clone());

				return;
			}

			let Type::Reference(ty_ref) = rec.ty.as_mut() else {
				unreachable!();
			};

			let lifetime = self.next_lifetime(reference.0.span());

			reference.1 = Some(lifetime.clone());
			ty_ref.lifetime = Some(lifetime);

			return;
		}

		if let Type::Reference(reference) = rec.ty.as_mut() {
			self.visit_type_reference_mut(reference);
		} else {
			visit_type_mut(self, rec.ty.as_mut());
		}
	}

	fn visit_type_impl_trait_mut(&mut self, impl_trait: &mut TypeImplTrait) {
		impl_trait.bounds.push(TypeParamBound::Lifetime(
			self.next_lifetime(impl_trait.span())
		));
	}

	fn visit_return_type_mut(&mut self, ret: &mut ReturnType) {
		if let ReturnType::Type(_, ty) = ret {
			if let Type::Reference(reference) = ty.as_mut() {
				self.visit_type_reference_mut(reference);
			}
		}
	}

	fn visit_signature_mut(&mut self, sig: &mut Signature) {
		for arg in &mut sig.inputs {
			self.visit_fn_arg_mut(arg);
		}

		self.visit_return_type_mut(&mut sig.output);
	}
}

fn capture_lifetimes(sig: &Signature, env_generics: Option<&Generics>) -> TokenStream {
	let mut addl_bounds = Punctuated::<TypeParamBound, Token![+]>::new();

	if let Some(generics) = env_generics {
		for param in generics.lifetimes() {
			let lifetime = &param.lifetime;

			addl_bounds.push(parse_quote! { ::xx_core::impls::Captures<#lifetime> });
		}
	}

	/* See https://github.com/danielhenrymantilla/fix_hidden_lifetime_bug.rs
	 * TODO: remove when trait aliases are stable
	 */
	for param in sig.generics.lifetimes() {
		let lifetime = &param.lifetime;

		addl_bounds.push(parse_quote! { ::xx_core::impls::Captures<#lifetime> });
	}

	addl_bounds.push(closure_lifetime_parsed());

	quote! { #addl_bounds }
}

fn add_lifetime(
	sig: &mut Signature, env_generics: Option<&Generics>, annotations: LifetimeAnnotations,
	is_trait: bool, capture_lifetime: bool
) -> TokenStream {
	if annotations == LifetimeAnnotations::None {
		return quote! {};
	}

	let mut op = AddLifetime::new(annotations);

	op.visit_signature_mut(sig);

	let closure_lifetime = closure_lifetime();
	let has_receiver = sig.receiver().is_some();

	let clause = sig
		.generics
		.where_clause
		.get_or_insert_with(|| WhereClause {
			where_token: Default::default(),
			predicates: Punctuated::new()
		});

	let mut add_bounds = |params: &Punctuated<GenericParam, Token![,]>| {
		for param in params {
			match param {
				GenericParam::Const(_) => (),
				GenericParam::Type(ty) => {
					let ident = &ty.ident;

					clause
						.predicates
						.push(parse_quote! { #ident: #closure_lifetime });
				}

				GenericParam::Lifetime(param) => {
					let lifetime = &param.lifetime;

					clause
						.predicates
						.push(parse_quote! { #lifetime: #closure_lifetime });
				}
			}
		}
	};

	add_bounds(&sig.generics.params);

	if let Some(generics) = env_generics {
		add_bounds(&generics.params);
	}

	let has_self_bound = clause.predicates.iter().any(|predicate| {
		let WherePredicate::Type(PredicateType {
			bounded_ty: Type::Path(TypePath { qself: None, path }),
			..
		}) = predicate
		else {
			return false;
		};

		path.get_ident().is_some_and(|ident| ident == "Self")
	});

	if !has_self_bound && is_trait {
		if has_receiver {
			clause
				.predicates
				.push(parse_quote! { Self: #closure_lifetime });
		} else {
			clause.predicates.push(parse_quote! { Self: 'static });
		}
	}

	sig.generics.params.push(closure_lifetime_parsed());

	for lifetime in &op.added_lifetimes {
		sig.generics.params.push(parse_quote! { #lifetime });
	}

	for lifetime in op
		.added_lifetimes
		.iter()
		.chain(op.explicit_lifetimes.iter())
	{
		clause
			.predicates
			.push(parse_quote! { #lifetime: #closure_lifetime });
	}

	if capture_lifetime {
		let lifetimes = capture_lifetimes(sig, env_generics);

		quote! { + #lifetimes }
	} else {
		quote! { + #closure_lifetime }
	}
}

pub fn make_tuple_of_types<T>(data: Vec<T>) -> TokenStream
where
	T: ToTokens
{
	let data: Punctuated<T, Token![,]> = data.into_iter().collect();

	if data.len() == 1 {
		data.to_token_stream()
	} else {
		quote_spanned! { data.span() => (#data) }
	}
}

fn build_tuples(
	inputs: &mut Punctuated<FnArg, Token![,]>, map: impl FnMut(&mut FnArg) -> (Type, Pat, Pat)
) -> (TokenStream, TokenStream, TokenStream) {
	let data: Vec<(Type, Pat, Pat)> = inputs.iter_mut().map(map).collect();

	(
		make_tuple_of_types(data.iter().map(|tp| tp.0.clone()).collect()),
		make_tuple_of_types(data.iter().map(|tp| tp.1.clone()).collect()),
		make_tuple_of_types(data.iter().map(|tp| tp.2.clone()).collect())
	)
}

fn make_args(args_pat_type: &Vec<(TokenStream, TokenStream)>) -> (TokenStream, TokenStream) {
	let mut args = Vec::new();
	let mut types = Vec::new();

	for (pat, ty) in args_pat_type {
		args.push(quote! { #pat: #ty });
		types.push(ty.clone());
	}

	(quote! { #(#args),* }, quote! { #(#types),* })
}

pub fn make_explicit_closure(
	func: &mut Function<'_>, args: &Vec<(TokenStream, TokenStream)>, closure_type: TokenStream,
	transform_return: impl Fn(TokenStream, TokenStream) -> TokenStream,
	annotations: LifetimeAnnotations
) -> Result<TokenStream> {
	add_lifetime(func.sig, func.env_generics, annotations, false, false);

	let (types, construct, destruct) = build_tuples(&mut func.sig.inputs, |arg| match arg {
		FnArg::Typed(pat) => {
			let destr = pat.pat.as_ref().clone();

			RemoveRefMut {}.visit_pat_mut(pat.pat.as_mut());

			(pat.ty.as_ref().clone(), pat.pat.as_ref().clone(), destr)
		}

		FnArg::Receiver(rec) => {
			let make_pat_ident = |ident: &str, copy_mut: bool| {
				let mutability = if copy_mut && rec.reference.is_none() {
					rec.mutability
				} else {
					None
				};

				Pat::Ident(PatIdent {
					attrs: rec.attrs.clone(),
					by_ref: None,
					mutability,
					ident: format_ident!("{}", ident, span = rec.span()),
					subpat: None
				})
			};

			(
				rec.ty.as_ref().clone(),
				make_pat_ident("self", false),
				make_pat_ident(SELF_IDENT, true)
			)
		}
	});

	let (args, _) = make_args(args);
	let return_type = get_return_type(&func.sig.output);
	let closure_return_type = transform_return(types.clone(), return_type.clone());

	func.sig.output = parse_quote! { -> #closure_return_type };

	if let Some(block) = &mut func.block {
		func.attrs.push(parse_quote!( #[inline(always)] ));

		ReplaceSelf {}.visit_block_mut(block);

		**block = parse_quote! {{
			#closure_type::new(
				#[allow(clippy::used_underscore_binding)] { #construct },
				| #destruct: #types, #args | -> #return_type #block
			)
		}};
	}

	Ok(closure_return_type)
}

#[allow(dead_code)]
pub enum OpaqueClosureType<T> {
	Fn(),
	Custom(T)
}

pub fn make_opaque_closure(
	func: &mut Function<'_>, args: &Vec<(TokenStream, TokenStream)>,
	transform_return: impl Fn(TokenStream) -> TokenStream,
	closure_type: OpaqueClosureType<impl Fn(TokenStream) -> (TokenStream, TokenStream)>,
	is_trait: bool, workaround: bool
) -> Result<TokenStream> {
	let addl_lifetimes = add_lifetime(
		func.sig,
		func.env_generics,
		LifetimeAnnotations::Auto,
		is_trait,
		workaround
	);

	let (args, args_types) = make_args(args);

	let return_type = &mut func.sig.output;
	let closure_return_type = transform_return(get_return_type(return_type));
	let (closure_return_type, trait_impl_wrap) = match closure_type {
		OpaqueClosureType::Custom(transform) => {
			let (trait_type, trait_impl) = transform(closure_return_type);

			(
				parse_quote_spanned! { trait_type.span() => impl #trait_type },
				Some(trait_impl)
			)
		}

		OpaqueClosureType::Fn() => (
			parse_quote_spanned! { closure_return_type.span() =>
				impl FnOnce( #args_types ) -> #closure_return_type
			},
			None
		)
	};

	if let Some(block) = &mut func.block {
		let mut closure = quote! { move | #args | #return_type #block };

		if let Some(wrap) = trait_impl_wrap {
			closure = quote! { #wrap::new(#closure) }
		}

		**block = parse_quote! {{ #closure }};
	}

	func.attrs
		.push(parse_quote! { #[allow(clippy::type_complexity)] });
	func.sig.output = parse_quote! { -> #closure_return_type #addl_lifetimes };

	Ok(closure_return_type)
}
