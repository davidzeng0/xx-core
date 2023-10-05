use proc_macro2::TokenStream;
use syn::{visit_mut::*, *, punctuated::Punctuated, spanned::Spanned};
use quote::{quote, ToTokens};

struct ReplaceSelf;

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
		ident.by_ref.take();
		ident.mutability.take();
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

fn build_tuples(inputs: &Punctuated<FnArg, Token![,]>, map: fn(&FnArg) -> (Type, Pat, Pat)) -> (TokenStream, TokenStream, TokenStream) {
	let data: Vec<(Type, Pat, Pat)> = inputs.iter().map(|arg| map(arg)).collect();

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

pub fn into_closure(
	attrs: &mut Vec<Attribute>,
	sig: &mut Signature,
	block: Option<&mut Block>,
	(args_types, args_destruct): (TokenStream, TokenStream),
	(closure_type, additional_generics): (TokenStream, Vec<Type>),
	no_generic_return_type: bool
) -> Result<TokenStream> {
	let return_type = get_return_type(&sig.output);

	let (types, construct, destruct) = build_tuples(&sig.inputs, |arg| match arg {
		FnArg::Typed(pat) => {
			let mut constr = pat.pat.as_ref().clone();

			RemoveRefMut {}.visit_pat_mut(&mut constr);

			(
				pat.ty.as_ref().clone(),
				constr,
				pat.pat.as_ref().clone()
			)
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

	let mut closure_type_generics = Punctuated::<Type, Token![,]>::new();

	closure_type_generics.push(parse_quote!{ #types });

	if !no_generic_return_type {
		closure_type_generics.push(parse_quote!{ #return_type });
	}

	additional_generics.into_iter().for_each(|ty| {
		closure_type_generics.push(ty);
	});

	let closure_return_type = quote! { #closure_type<#closure_type_generics> };

	sig.output = parse_quote! {
		-> #closure_return_type
	};

	if let Some(block) = block {
		attrs.push(parse_quote!( #[inline(always)] ));

		ReplaceSelf {}.visit_block_mut(block);

		*block = parse_quote! {{
			let run = |
				#destruct: #types,
				#args_destruct: #args_types
			| -> #return_type #block;

			#closure_type::new(#construct, run)
		}};
	}

	Ok(closure_return_type)
}