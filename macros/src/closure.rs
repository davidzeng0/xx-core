use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{punctuated::Punctuated, spanned::Spanned, visit_mut::*, *};

pub struct ReplaceSelf;

impl VisitMut for ReplaceSelf {
	fn visit_ident_mut(&mut self, ident: &mut Ident) {
		visit_ident_mut(self, ident);

		if ident == "self" {
			*ident = Ident::new("__xx_closure_internal_self", ident.span());
		}
	}
}

struct RemoveRefMut;

impl VisitMut for RemoveRefMut {
	fn visit_pat_ident_mut(&mut self, ident: &mut PatIdent) {
		visit_pat_ident_mut(self, ident);

		ident.by_ref.take();
		ident.mutability.take();
	}
}

struct AddLifetime {
	modified: bool
}

impl VisitMut for AddLifetime {
	fn visit_type_reference_mut(&mut self, reference: &mut TypeReference) {
		visit_type_reference_mut(self, reference);

		if reference.lifetime.is_some() {
			return;
		}

		self.modified = true;

		reference.lifetime = Some(parse_quote! { 'xx_closure_internal_lifetime });
	}

	fn visit_lifetime_mut(&mut self, lifetime: &mut Lifetime) {
		visit_lifetime_mut(self, lifetime);

		if lifetime.ident != "_" {
			return;
		}

		self.modified = true;

		lifetime.ident = Ident::new("xx_closure_internal_lifetime", lifetime.ident.span());
	}

	fn visit_receiver_mut(&mut self, rec: &mut Receiver) {
		visit_receiver_mut(self, rec);

		if let Some(reference) = &mut rec.reference {
			if reference.1.is_some() {
				return;
			}

			self.modified = true;

			reference.1 = Some(parse_quote! { 'xx_closure_internal_lifetime });
		}
	}
}

fn add_lifetime(sig: &mut Signature) -> bool {
	let mut op = AddLifetime { modified: false };

	op.visit_signature_mut(sig);
	op.modified
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

fn make_args(
	args_vars: Vec<TokenStream>, args_types: Vec<TokenStream>
) -> (TokenStream, TokenStream) {
	let mut args = Vec::new();

	for i in 0..args_vars.len() {
		let var = &args_vars[i];
		let ty = &args_types[i];

		args.push(quote! { #var: #ty });
	}

	(quote! { #(#args),* }, quote! { #(#args_types),* })
}

pub fn into_closure(
	attrs: &mut Vec<Attribute>, sig: &mut Signature, block: Option<&mut Block>,
	args_vars: Vec<TokenStream>, args_types: Vec<TokenStream>, closure_type: TokenStream,
	transform_return: impl Fn(TokenStream) -> TokenStream
) -> Result<TokenStream> {
	let return_type = get_return_type(&sig.output);

	if add_lifetime(sig) {
		sig.generics
			.params
			.push(parse_quote! { 'xx_closure_internal_lifetime });
	}

	let (types, construct, destruct) = build_tuples(&mut sig.inputs, |arg| match arg {
		FnArg::Typed(pat) => {
			let mut constr = pat.pat.as_ref().clone();

			RemoveRefMut {}.visit_pat_mut(&mut constr);

			(pat.ty.as_ref().clone(), constr, pat.pat.as_ref().clone())
		}

		FnArg::Receiver(rec) => {
			let make_pat_ident = |ident: &str| {
				Pat::Ident(PatIdent {
					attrs: rec.attrs.clone(),
					by_ref: None,
					mutability: None,
					ident: Ident::new(ident, rec.span()),
					subpat: None
				})
			};

			(
				rec.ty.as_ref().clone(),
				make_pat_ident("self"),
				make_pat_ident("__xx_closure_internal_self")
			)
		}
	});

	RemoveRefMut {}.visit_signature_mut(sig);

	let closure_return_type = transform_return(types.clone());

	let (args, _) = make_args(args_vars, args_types);

	sig.output = parse_quote! {
		-> #closure_return_type
	};

	if let Some(block) = block {
		attrs.push(parse_quote!( #[inline(always)] ));

		ReplaceSelf {}.visit_block_mut(block);

		*block = parse_quote! {{
			let run = |
				#destruct: #types,
				#args
			| -> #return_type #block;

			#closure_type::new(#construct, run)
		}};
	}

	Ok(closure_return_type)
}

pub fn into_basic_closure(
	attrs: &mut Vec<Attribute>, sig: &mut Signature, block: Option<&mut Block>,
	args_vars: Vec<TokenStream>, args_types: Vec<TokenStream>,
	transform_return: impl Fn(TokenStream) -> TokenStream,
	wrap: Option<impl Fn(TokenStream) -> (TokenStream, TokenStream)>
) -> Result<TokenStream> {
	let lifetime_added = add_lifetime(sig);

	if lifetime_added {
		sig.generics
			.params
			.push(parse_quote! { 'xx_closure_internal_lifetime });
	}

	let return_type = &mut sig.output;

	let (args, args_types) = make_args(args_vars, args_types);

	let return_tokens = transform_return(get_return_type(&return_type));
	let wrap = wrap.map(|transform| transform(return_tokens.clone()));
	let return_tokens = if let Some(wrap) = wrap.clone() {
		let trait_type = wrap.0;

		quote! { impl #trait_type }
	} else {
		quote! { impl FnOnce( #args_types ) -> #return_tokens }
	};

	if let Some(block) = block {
		attrs.push(parse_quote!( #[inline(always)] ));

		let mut closure = quote! { move |#args| #return_type #block };

		if wrap.is_some() {
			let trait_impl = wrap.unwrap().1;

			closure = quote! { #trait_impl::new(#closure) }
		}

		*block = parse_quote! {{
			#closure
		}};
	}

	let lifetime = if lifetime_added {
		quote! { + 'xx_closure_internal_lifetime }
	} else {
		quote! {}
	};

	sig.output = parse_quote! {
		-> #return_tokens #lifetime
	};

	Ok(return_tokens)
}
