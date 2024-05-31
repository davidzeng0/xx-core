use super::*;

pub fn error(_: TokenStream, item: TokenStream) -> TokenStream {
	let mut input: DeriveInput = match parse2(item) {
		Ok(input) => input,
		Err(err) => return err.to_compile_error()
	};

	let name = &input.ident;

	input.attrs.push(parse_quote! {
		#[derive(
			::xx_core::error::internal::thiserror::Error,
			::std::fmt::Debug
		)]
	});

	input.attrs.push(parse_quote! {
		#[allow(missing_copy_implementations)]
	});

	let eq = if matches!(input.data, Data::Enum(_)) {
		Some(quote! {
			impl ::std::cmp::PartialEq for #name {
				fn eq(&self, other: &Self) -> bool {
					::std::mem::discriminant(self) == ::std::mem::discriminant(other)
				}
			}
		})
	} else {
		None
	};

	quote! {
		#input

		impl ::xx_core::error::internal::IntoError for #name {}

		#eq
	}
}
