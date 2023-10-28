use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
	parse::{Parse, ParseStream},
	punctuated::Punctuated,
	*
};

struct Function {
	attrs: Vec<Attribute>,
	ident: Ident,
	vis: Visibility,
	sig: Signature
}

struct WrapperFunctions {
	inner: Expr,
	inner_mut: Expr,
	functions: Vec<Function>
}

impl Parse for WrapperFunctions {
	fn parse(input: ParseStream) -> Result<Self> {
		let mut inner = None;
		let mut inner_mut = None;

		for _ in 0..2 {
			let mutability = input.peek(Token![mut]);

			if mutability {
				input.parse::<Token![mut]>()?;
			}

			if !input.peek(Ident) {
				break;
			}

			let ident: Ident = input.parse()?;

			if ident != "inner" {
				return Err(input.error("unexpected ident"));
			}

			input.parse::<Token![=]>()?;

			let rhs: Expr = input.parse()?;

			input.parse::<Token![;]>()?;

			if mutability {
				inner_mut = Some(rhs);
			} else {
				inner = Some(rhs);
			}
		}

		if inner.is_none() {
			inner = inner_mut.clone();
		}

		if inner_mut.is_none() {
			inner_mut = inner.clone();
		}

		if inner.is_none() {
			return Err(input.error("expected an inner expression"));
		}

		let inner = inner.unwrap();
		let inner_mut = inner_mut.unwrap();

		let mut functions = Vec::new();

		while !input.is_empty() {
			let attrs = input.call(Attribute::parse_outer)?;
			let ident: Option<Ident> = input.parse()?;

			if ident.is_some() {
				input.parse::<Token![=]>()?;
			}

			let vis: Visibility = input.parse()?;
			let sig: Signature = input.parse()?;

			let ident = if let Some(ident) = ident {
				ident
			} else {
				sig.ident.clone()
			};

			input.parse::<Token![;]>()?;
			functions.push(Function { attrs, ident, vis, sig });
		}

		Ok(Self { inner, inner_mut, functions })
	}
}

pub fn get_pats(sig: &Signature) -> Punctuated<Pat, Token![,]> {
	let mut pats = Punctuated::new();

	for arg in sig.inputs.iter() {
		if let FnArg::Typed(arg) = arg {
			pats.push(arg.pat.as_ref().clone());
		}
	}

	pats
}

impl WrapperFunctions {
	pub fn expand(&self) -> TokenStream {
		let mut fns = Vec::new();

		for function in &self.functions {
			let mutable = function
				.sig
				.receiver()
				.is_some_and(|rec| rec.mutability.is_some());
			let pats = get_pats(&function.sig);

			let ident = &function.sig.ident;

			let maybe_await = if function.sig.asyncness.is_some() {
				quote! { .await }
			} else {
				quote! {}
			};

			let inner = if mutable {
				&self.inner_mut
			} else {
				&self.inner
			};

			let mut sig = function.sig.clone();

			sig.ident = function.ident.clone();

			let call = quote! { (#inner).#ident (#pats) #maybe_await };

			let mut attrs = function.attrs.clone();

			attrs.push(parse_quote! { #[inline(always )] });

			fns.push(ItemFn {
				attrs,
				vis: function.vis.clone(),
				sig,
				block: parse_quote! {{ #call }}
			});
		}

		let mut ts = TokenStream::new();

		for func in fns {
			func.to_tokens(&mut ts);
		}

		ts
	}
}

pub fn wrapper_functions(item: TokenStream) -> TokenStream {
	let functions = match parse2::<WrapperFunctions>(item) {
		Ok(functions) => functions,
		Err(err) => return err.to_compile_error()
	};

	functions.expand()
}
