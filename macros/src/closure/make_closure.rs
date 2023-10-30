use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::{parse::Parse, punctuated::Punctuated, spanned::Spanned, visit_mut::*, *};

use super::transform::Function;

pub struct ReplaceSelf;

impl VisitMut for ReplaceSelf {
	fn visit_ident_mut(&mut self, ident: &mut Ident) {
		visit_ident_mut(self, ident);

		if ident == "self" {
			*ident = format_ident!("__xx_internal_closure_self", span = ident.span());
		}
	}
}

pub struct RemoveRefMut;

impl VisitMut for RemoveRefMut {
	fn visit_pat_ident_mut(&mut self, ident: &mut PatIdent) {
		visit_pat_ident_mut(self, ident);

		ident.by_ref.take();
		ident.mutability.take();
	}
}

#[derive(Default)]
struct AddLifetime {
	added_lifetimes: Vec<Lifetime>,
	modified: bool
}

fn closure_lifetime<R: Parse>() -> R {
	parse_quote! { '__xx_internal_closure_lifetime }
}

impl AddLifetime {
	fn next_lifetime(&mut self, span: Span) -> Lifetime {
		let lifetime = Lifetime::new(
			&format!(
				"'__xx_internal_closure_lifetime_{}",
				self.added_lifetimes.len() + 1
			),
			span
		);

		self.added_lifetimes.push(lifetime.clone());

		lifetime
	}
}

impl VisitMut for AddLifetime {
	fn visit_type_reference_mut(&mut self, reference: &mut TypeReference) {
		if reference.lifetime.is_some() {
			return;
		}

		match reference.elem.as_ref() {
			Type::ImplTrait(_) => (),
			_ => visit_type_reference_mut(self, reference)
		}

		reference.lifetime = Some(self.next_lifetime(reference.span()));

		self.modified = true;
	}

	fn visit_lifetime_mut(&mut self, lifetime: &mut Lifetime) {
		visit_lifetime_mut(self, lifetime);

		if lifetime.ident == "_" {
			*lifetime = self.next_lifetime(lifetime.span());

			self.modified = true;
		}
	}

	fn visit_receiver_mut(&mut self, rec: &mut Receiver) {
		if let Some(reference) = &mut rec.reference {
			if reference.1.is_none() {
				let lifetime = self.next_lifetime(reference.0.span());

				reference.1 = Some(lifetime.clone());

				self.modified = true;
			}
		} else if let Type::Reference(reference) = rec.ty.as_mut() {
			self.visit_type_reference_mut(reference);
		} else {
			visit_type_mut(self, rec.ty.as_mut());
		}
	}

	fn visit_type_impl_trait_mut(&mut self, impl_trait: &mut TypeImplTrait) {
		impl_trait.bounds.push(TypeParamBound::Lifetime(
			self.next_lifetime(impl_trait.span())
		));

		self.modified = true;
	}

	fn visit_type_param_mut(&mut self, param: &mut TypeParam) {
		let lifetime = self.next_lifetime(param.span());

		if param.bounds.iter().any(|bound| match bound {
			TypeParamBound::Lifetime(_) => true,
			_ => false
		}) {
			return;
		}

		param.colon_token = Some(Default::default());
		param.bounds.push(TypeParamBound::Lifetime(lifetime));

		self.modified = true;
	}

	fn visit_lifetime_param_mut(&mut self, lifetime: &mut LifetimeParam) {
		visit_lifetime_param_mut(self, lifetime);

		lifetime.colon_token = Some(Default::default());
		lifetime.bounds.push(closure_lifetime());

		self.modified = true;
	}
}

/// See https://github.com/danielhenrymantilla/fix_hidden_lifetime_bug.rs
fn lifetime_workaround(sig: &mut Signature, env_generics: &Option<&Generics>) -> TokenStream {
	let mut addl_bounds = Punctuated::<TypeParamBound, Token![+]>::new();

	env_generics.map(|generics| {
		generics.params.iter().for_each(|param| match param {
			GenericParam::Const(_) => (),
			GenericParam::Type(_) => (),
			GenericParam::Lifetime(param) => {
				let lifetime = &param.lifetime;

				addl_bounds
					.push(parse_quote! { xx_core::macros::closure::lifetime::Captures<#lifetime> });
			}
		})
	});

	/* this is apparently necessary */
	for param in sig.generics.lifetimes() {
		let lifetime = &param.lifetime;

		if lifetime != &closure_lifetime() {
			addl_bounds
				.push(parse_quote! { xx_core::macros::closure::lifetime::Captures<#lifetime> });
		}
	}

	addl_bounds.push(closure_lifetime());

	quote! { #addl_bounds }
}

struct ReturnLifetime;

impl VisitMut for ReturnLifetime {
	fn visit_type_reference_mut(&mut self, reference: &mut TypeReference) {
		visit_type_reference_mut(self, reference);

		if reference.lifetime.is_some() {
			return;
		}

		reference.lifetime = Some(closure_lifetime());
	}
}

pub fn add_lifetime(sig: &mut Signature, env_generics: &Option<&Generics>) -> TokenStream {
	let mut op = AddLifetime::default();

	for arg in &mut sig.inputs {
		op.visit_fn_arg_mut(arg);
	}

	if !op.modified {
		return quote! {};
	}

	ReturnLifetime {}.visit_return_type_mut(&mut sig.output);

	op.visit_signature_mut(sig);
	sig.generics.params.push(closure_lifetime());

	let clause = sig
		.generics
		.where_clause
		.get_or_insert_with(|| WhereClause {
			where_token: Default::default(),
			predicates: Punctuated::new()
		});

	let mut self_bounds = Punctuated::<Lifetime, Token![+]>::new();

	env_generics.map(|generics| {
		generics.params.iter().for_each(|param| match param {
			GenericParam::Const(_) => (),
			GenericParam::Type(_) => (),
			GenericParam::Lifetime(param) => {
				let lifetime = &param.lifetime;

				clause
					.predicates
					.push(parse_quote! { #lifetime: '__xx_internal_closure_lifetime });
				self_bounds.push(lifetime.clone());
			}
		})
	});

	for lifetime in &op.added_lifetimes {
		sig.generics.params.push(parse_quote! { #lifetime });
		clause
			.predicates
			.push(parse_quote! { #lifetime: '__xx_internal_closure_lifetime });
	}

	if true {
		let lifetimes = lifetime_workaround(sig, env_generics);

		quote! { + #lifetimes }
	} else {
		quote! { + '__xx_internal_closure_lifetime }
	}
}

pub fn make_tuple_type<T: ToTokens>(data: Vec<T>) -> TokenStream {
	let data: Punctuated<T, Token![,]> = data.into_iter().map(|arg| arg).collect();

	if data.len() == 1 {
		quote! { #data }
	} else {
		quote! { (#data) }
	}
}

fn build_tuples(
	inputs: &mut Punctuated<FnArg, Token![,]>, mut map: impl FnMut(&mut FnArg) -> (Type, Pat, Pat)
) -> (TokenStream, TokenStream, TokenStream) {
	let data: Vec<(Type, Pat, Pat)> = inputs.iter_mut().map(|arg| map(arg)).collect();

	(
		make_tuple_type(data.iter().map(|tp| tp.0.clone()).collect()),
		make_tuple_type(data.iter().map(|tp| tp.1.clone()).collect()),
		make_tuple_type(data.iter().map(|tp| tp.2.clone()).collect())
	)
}

pub fn get_return_type(ret: &ReturnType) -> TokenStream {
	if let ReturnType::Type(_, ty) = ret {
		quote! { #ty }
	} else {
		quote! { () }
	}
}

fn make_args(args_pat_type: Vec<(TokenStream, TokenStream)>) -> (TokenStream, TokenStream) {
	let mut args = Vec::new();
	let mut types = Vec::new();

	for (pat, ty) in &args_pat_type {
		args.push(quote! { #pat: #ty });
		types.push(ty.clone());
	}

	(quote! { #(#args),* }, quote! { #(#types),* })
}

pub fn into_typed_closure(
	func: &mut Function, args: Vec<(TokenStream, TokenStream)>, closure_type: TokenStream,
	transform_return: impl Fn(TokenStream, TokenStream) -> TokenStream
) -> Result<TokenStream> {
	add_lifetime(&mut func.sig, &func.env_generics);

	let (types, construct, destruct) = build_tuples(&mut func.sig.inputs, |arg| match arg {
		FnArg::Typed(pat) => {
			let destr = pat.pat.as_ref().clone();

			RemoveRefMut {}.visit_pat_mut(pat.pat.as_mut());

			(pat.ty.as_ref().clone(), pat.pat.as_ref().clone(), destr)
		}

		FnArg::Receiver(rec) => {
			let make_pat_ident = |ident: &str| {
				Pat::Ident(PatIdent {
					attrs: rec.attrs.clone(),
					by_ref: None,
					mutability: None,
					ident: format_ident!("{}", ident, span = rec.span()),
					subpat: None
				})
			};

			(
				rec.ty.as_ref().clone(),
				make_pat_ident("self"),
				make_pat_ident("__xx_internal_closure_self")
			)
		}
	});

	let (args, _) = make_args(args);

	let return_type = get_return_type(&func.sig.output);
	let closure_return_type = transform_return(types.clone(), return_type.clone());

	func.sig.output = parse_quote! { -> #closure_return_type };

	if let Some(block) = &mut func.block {
		func.attrs.push(parse_quote!( #[inline(always)] ));

		ReplaceSelf {}.visit_block_mut(*block);

		**block = parse_quote! {{
			let run = |
				#destruct: #types,
				#args
			| -> #return_type #block;

			#closure_type::new(#construct, run)
		}};
	}

	Ok(closure_return_type)
}

#[allow(dead_code)]
pub enum OpaqueClosureType<T> {
	Fn(),
	Custom(T)
}

pub fn into_opaque_closure(
	func: &mut Function, args: Vec<(TokenStream, TokenStream)>,
	transform_return: impl Fn(TokenStream) -> TokenStream,
	closure_type: OpaqueClosureType<impl Fn(TokenStream) -> (TokenStream, TokenStream)>
) -> Result<TokenStream> {
	let addl_lifetimes = add_lifetime(&mut func.sig, &func.env_generics);

	let (args, args_types) = make_args(args);

	let return_type = &mut func.sig.output;
	let closure_return_type = transform_return(get_return_type(&return_type));
	let (closure_return_type, trait_impl_wrap) = match closure_type {
		OpaqueClosureType::Custom(transform) => {
			let (trait_type, trait_impl) = transform(closure_return_type);

			(quote! { impl #trait_type }, Some(trait_impl))
		}

		OpaqueClosureType::Fn() => (
			quote! { impl FnOnce( #args_types ) -> #closure_return_type },
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

	func.sig.output = parse_quote! { -> #closure_return_type #addl_lifetimes };

	Ok(closure_return_type)
}
