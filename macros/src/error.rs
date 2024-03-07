use super::*;

pub fn compact_error(_: TokenStream, item: TokenStream) -> TokenStream {
	let mut error: ItemEnum = match parse2(item) {
		Ok(variants) => variants,
		Err(err) => return err.to_compile_error()
	};

	error.attrs.push(parse_quote! { #[repr(u32)] });
	error
		.attrs
		.push(parse_quote! { #[derive(Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Debug)] });

	let mut variants = Vec::new();
	let mut kinds = Vec::new();
	let mut strings = Vec::new();

	for (index, variant) in error.variants.iter_mut().enumerate() {
		let (eq, expr) = match variant.discriminant.take() {
			None => return Error::new_spanned(variant, "Expected error data").to_compile_error(),
			Some(discr) => discr
		};

		let Expr::Tuple(tuple) = expr else {
			return Error::new_spanned(expr, "Expected a tuple").to_compile_error();
		};

		if tuple.elems.len() != 2 {
			return Error::new_spanned(tuple, "Expected error kind and message").to_compile_error();
		}

		variants.push(variant.ident.clone());
		kinds.push(tuple.elems[0].clone());
		strings.push(tuple.elems[1].clone());

		let index = index as u32;

		variant.discriminant = Some((eq, parse_quote! { #index }));
	}

	let ident = &error.ident;

	quote! {
		#error

		impl ::xx_core::error::CompactError for #ident {
			const STRINGS: &'static [&'static str] = &[
				::std::stringify!(#ident),
				#(::std::stringify!(#variants), #strings),*
			];

			fn kind(&self) -> ErrorKind {
				match self {
					#(Self::#variants => #kinds),*
				}
			}

			fn ordinal(&self) -> u32 {
				*self as u32
			}

			unsafe fn from_ordinal_unchecked(ordinal: u32) -> Self {
				::std::mem::transmute(ordinal)
			}
		}
	}
}
