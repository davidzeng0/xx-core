use super::*;

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
			if !input.peek(Ident) && !(input.peek(Token![mut]) && input.peek2(Ident)) {
				break;
			}

			let mutability: Option<Token![mut]> = input.parse()?;
			let ident: Ident = input.parse()?;
			let rhs: Expr;

			if ident != "inner" {
				return Err(input.error("unexpected ident"));
			}

			input.parse::<Token![=]>()?;
			rhs = input.parse()?;
			input.parse::<Token![;]>()?;

			if mutability.is_some() {
				inner_mut = Some(rhs);
			} else {
				inner = Some(rhs);
			}
		}

		if inner.is_none() && inner_mut.is_none() {
			return Err(input.error("expected an inner expression"));
		}

		let inner = inner.unwrap_or_else(|| inner_mut.clone().unwrap());
		let inner_mut = inner_mut.unwrap_or(inner.clone());

		let mut functions = Vec::new();

		while !input.is_empty() {
			let attrs = input.call(Attribute::parse_outer)?;
			let ident: Option<Ident> = input.parse()?;

			if ident.is_some() {
				input.parse::<Token![=]>()?;
			}

			let vis: Visibility = input.parse()?;
			let sig: Signature = input.parse()?;

			let ident = ident.unwrap_or(sig.ident.clone());

			input.parse::<Token![;]>()?;
			functions.push(Function { attrs, ident, vis, sig });
		}

		Ok(Self { inner, inner_mut, functions })
	}
}

impl WrapperFunctions {
	pub fn expand(&self) -> TokenStream {
		let mut fns = Vec::new();

		for function in &self.functions {
			let mutable = function
				.sig
				.receiver()
				.is_some_and(|rec| rec.mutability.is_some());
			let pats = get_args(&function.sig, false);
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
			let mut attrs = function.attrs.clone();
			let mut stmts = Vec::new();

			stmts.push(quote! { (#inner).#ident (#pats) #maybe_await });
			attrs.push(parse_quote! { #[inline(always )] });
			sig.ident = function.ident.clone();

			if let Some(position) = attrs.iter().position(|attr| match &attr.meta {
				Meta::Path(Path { leading_colon: None, segments }) => {
					segments.len() == 1 &&
						segments[0].arguments.is_none() &&
						segments[0].ident == "chain"
				}

				_ => false
			}) {
				attrs.remove(position);
				stmts.push(quote! { ; self });
			}

			fns.push(ItemFn {
				attrs,
				vis: function.vis.clone(),
				sig,
				block: parse_quote! {{ #(#stmts)* }}
			});
		}

		quote! { #(#fns)* }
	}
}

pub fn wrapper_functions(item: TokenStream) -> TokenStream {
	let functions = match parse2::<WrapperFunctions>(item) {
		Ok(functions) => functions,
		Err(err) => return err.to_compile_error()
	};

	functions.expand()
}
