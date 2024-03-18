use super::*;

fn expand(item: TokenStream) -> Result<TokenStream> {
	let mut error: ItemEnum = parse2(item)?;

	error.attrs.push(parse_quote! { #[repr(u16)] });
	error
		.attrs
		.push(parse_quote! { #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)] });

	let mut variants = Vec::new();
	let mut kinds = Vec::new();
	let mut strings = Vec::new();
	let mut ordinals = Vec::new();

	for (index, variant) in error.variants.iter_mut().enumerate() {
		let (eq, tuple) = match variant.discriminant.take() {
			Some((eq, Expr::Tuple(tuple))) => (eq, tuple),
			_ => return Err(Error::new_spanned(variant, "Expected a tuple"))
		};

		if tuple.elems.len() != 2 {
			return Err(Error::new_spanned(tuple, "Expected error kind and message"));
		}

		variants.push(variant.ident.clone());
		kinds.push(tuple.elems[0].clone());
		strings.push(tuple.elems[1].clone());

		#[allow(clippy::cast_possible_truncation)]
		let index = index as u16;

		ordinals.push(index);
		variant.discriminant = Some((eq, parse_quote! { #index }));
	}

	let ident = &error.ident;

	Ok(quote! {
		#error

		unsafe impl ::xx_core::error::CompactError for #ident {
			const STRINGS: &'static [&'static str] = &[
				::std::stringify!(#ident),
				#(
					::std::stringify!(#variants),
					#strings
				),*
			];

			fn kind(self) -> ErrorKind {
				match self {
					#(Self::#variants => #kinds),*
				}
			}

			fn message(self) -> &'static str {
				match self {
					#(Self::#variants => #strings),*
				}
			}

			fn ordinal(self) -> u16 {
				self as u16
			}

			fn from_ordinal(ordinal: u16) -> Option<Self> {
				match ordinal {
					#(#ordinals => Some(Self::#variants),)*
					_ => None
				}
			}

			unsafe fn from_ordinal_unchecked(ordinal: u16) -> Self {
				::std::mem::transmute(ordinal)
			}
		}
	})
}

pub fn compact_error(_: TokenStream, item: TokenStream) -> TokenStream {
	match expand(item) {
		Ok(tokens) => tokens,
		Err(err) => err.to_compile_error()
	}
}
