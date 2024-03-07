use super::*;

#[derive(Clone)]
pub enum Functions {
	Fn(ImplItemFn),
	TraitFn(TraitItemFn),
	Trait(ItemTrait),
	Impl(ItemImpl)
}

impl Parse for Functions {
	fn parse(item: ParseStream) -> Result<Self> {
		let lookahead = item.fork();

		lookahead.call(Attribute::parse_outer)?;
		lookahead.parse::<Visibility>()?;
		lookahead.parse::<Option<Token![default]>>()?;
		lookahead.parse::<Option<Token![unsafe]>>()?;

		if lookahead.peek(Token![auto]) || lookahead.peek(Token![trait]) {
			return item.parse().map(|item| Self::Trait(item));
		}

		if lookahead.peek(Token![impl]) {
			return item.parse().map(|item| Self::Impl(item));
		}

		lookahead.parse::<Option<Token![const]>>()?;
		lookahead.parse::<Option<Token![async]>>()?;
		lookahead.parse::<Option<Token![unsafe]>>()?;
		lookahead.parse::<Option<Abi>>()?;

		if !lookahead.parse::<Token![fn]>().is_ok() {
			return Err(lookahead.error("Expected a function, trait, or impl"));
		}

		if let Ok(item) = item.parse() {
			return Ok(Self::Fn(item));
		}

		item.parse().map(|item| Self::TraitFn(item))
	}
}

pub fn transform_functions(
	item: Functions, callback: impl Fn(&mut Function) -> Result<()>,
	allowed: impl FnOnce(&Functions) -> bool
) -> Result<TokenStream> {
	if !allowed(&item) {
		let tokens = match &item {
			Functions::Fn(func) => func.to_token_stream(),
			Functions::TraitFn(func) => func.to_token_stream(),
			Functions::Impl(item) => item.to_token_stream(),
			Functions::Trait(item) => item.to_token_stream()
		};

		return Err(Error::new_spanned(tokens, "Unexpected declaration"));
	}

	Ok(match item {
		Functions::Fn(mut func) => {
			callback(&mut Function {
				is_root: true,
				attrs: &mut func.attrs,
				env_generics: None,
				sig: &mut func.sig,
				block: Some(&mut func.block)
			})?;

			func.to_token_stream()
		}

		Functions::TraitFn(mut func) => {
			callback(&mut Function {
				is_root: true,
				attrs: &mut func.attrs,
				env_generics: None,
				sig: &mut func.sig,
				block: func.default.as_mut()
			})?;

			func.to_token_stream()
		}

		Functions::Impl(mut item) => {
			for impl_item in &mut item.items {
				let ImplItem::Fn(func) = impl_item else {
					continue;
				};

				callback(&mut Function {
					is_root: false,
					attrs: &mut func.attrs,
					env_generics: Some(&item.generics),
					sig: &mut func.sig,
					block: Some(&mut func.block)
				})?;
			}

			item.to_token_stream()
		}

		Functions::Trait(mut item) => {
			for trait_item in &mut item.items {
				let TraitItem::Fn(func) = trait_item else {
					continue;
				};

				callback(&mut Function {
					is_root: false,
					attrs: &mut func.attrs,
					env_generics: Some(&item.generics),
					sig: &mut func.sig,
					block: func.default.as_mut()
				})?;
			}

			item.to_token_stream()
		}
	})
}

pub fn transform_fn(
	item: TokenStream, callback: impl Fn(&mut Function) -> Result<()>,
	allowed: impl FnOnce(&Functions) -> bool
) -> TokenStream {
	let parsed = match parse2::<Functions>(item.clone()) {
		Ok(parsed) => parsed,
		Err(err) => return err.to_compile_error()
	};

	match transform_functions(parsed, callback, allowed) {
		Ok(ts) => ts,
		Err(err) => err.to_compile_error()
	}
}
