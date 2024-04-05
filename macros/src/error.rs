use super::*;

pub fn error(_: TokenStream, item: TokenStream) -> TokenStream {
	let mut input: DeriveInput = match parse2(item) {
		Ok(input) => input,
		Err(err) => return err.to_compile_error()
	};

	let name = &input.ident;

	input.attrs.push(parse_quote! {
		#[derive(
			::xx_core::error::re_exports::thiserror::Error,
			::std::fmt::Debug
		)]
	});

	input.attrs.push(parse_quote! {
		#[allow(missing_copy_implementations)]
	});

	quote! {
		#input

		impl ::xx_core::error::IntoError for #name {}
	}
}
