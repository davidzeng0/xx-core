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
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let mut inner = None;
		let mut inner_mut = None;

		for _ in 0..2 {
			/* reason: readability */
			#[allow(clippy::nonminimal_bool)]
			if !input.peek(Ident) && !(input.peek(Token![mut]) && input.peek2(Ident)) {
				break;
			}

			let mutability: Option<Token![mut]> = input.parse()?;
			let ident: Ident = input.parse()?;

			#[allow(clippy::needless_late_init)]
			let rhs: Expr;

			if ident != "inner" {
				return Err(Error::new_spanned(ident, "Expected `inner`"));
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
			return Err(input.error("Expected an inner expression"));
		}

		let inner = inner.unwrap_or_else(|| inner_mut.clone().unwrap());
		let inner_mut = inner_mut.unwrap_or_else(|| inner.clone());

		let mut functions = Vec::new();

		while !input.is_empty() {
			let attrs = input.call(Attribute::parse_outer)?;
			let ident: Option<Ident> = input.parse()?;

			if ident.is_some() {
				input.parse::<Token![=]>()?;
			}

			let vis: Visibility = input.parse()?;
			let sig: Signature = input.parse()?;

			let ident = ident.unwrap_or_else(|| sig.ident.clone());

			input.parse::<Token![;]>()?;
			functions.push(Function { attrs, ident, vis, sig });
		}

		Ok(Self { inner, inner_mut, functions })
	}
}

impl WrapperFunctions {
	fn expand(&self) -> TokenStream {
		let mut fns = Vec::new();

		for function in &self.functions {
			let mut call = Vec::new();

			let inner = if function
				.sig
				.receiver()
				.is_some_and(|rec| rec.mutability.is_some())
			{
				&self.inner_mut
			} else {
				&self.inner
			};

			call.push(quote_spanned! { function.sig.span() => (#inner) });

			let ident = &function.sig.ident;
			let pats = get_args(&function.sig.inputs, false);

			call.push(quote_spanned! { function.sig.span() => .#ident });
			call.push(quote_spanned! { pats.span() => (#pats) });

			if function.sig.asyncness.is_some() {
				call.push(quote_spanned! { function.sig.span() => .await });
			}

			let mut sig = function.sig.clone();
			let mut attrs = function.attrs.clone();
			let mut stmts = Vec::new();

			attrs.push(parse_quote! { #[inline(always)] });
			attrs.push(parse_quote! { #[allow(unsafe_op_in_unsafe_fn)] });
			stmts.push(quote! { #(#call)* });
			sig.ident = function.ident.clone();

			if let Some(attr) = remove_attr_path(&mut attrs, "chain") {
				stmts.push(quote_spanned! { attr.span() => ; self });
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
	try_expand(|| parse2::<WrapperFunctions>(item).map(|functions| functions.expand()))
}
