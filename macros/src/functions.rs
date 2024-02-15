use super::*;

pub struct Function<'a> {
	pub is_root: bool,
	pub attrs: &'a mut Vec<Attribute>,
	pub env_generics: Option<&'a Generics>,
	pub sig: &'a mut Signature,
	pub block: Option<&'a mut Block>
}

pub fn get_args(sig: &Signature, include_receiver: bool) -> Punctuated<Expr, Token![,]> {
	let mut args = Punctuated::new();

	for arg in sig.inputs.iter() {
		match arg {
			FnArg::Typed(arg) => {
				let mut pat = arg.pat.as_ref().clone();

				RemoveRefMut {}.visit_pat_mut(&mut pat);

				args.push(parse_quote! { #pat });
			}

			FnArg::Receiver(rec) if include_receiver => {
				let token = &rec.self_token;

				args.push(parse_quote! { #token });
			}

			_ => {}
		}
	}

	args
}

pub fn remove_attr(attrs: &mut Vec<Attribute>, target: &str) -> bool {
	if let Some(index) = attrs.iter().position(|attr| match &attr.meta {
		Meta::Path(path) => path.get_ident().is_some_and(|ident| ident == target),
		_ => false
	}) {
		attrs.remove(index);

		true
	} else {
		false
	}
}
