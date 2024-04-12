use super::*;

pub struct Function<'a> {
	pub is_root: bool,
	pub attrs: &'a mut Vec<Attribute>,
	pub env_generics: Option<&'a Generics>,
	pub sig: &'a mut Signature,
	pub block: Option<&'a mut Block>
}

impl<'a> Function<'a> {
	pub fn from_item_fn(
		is_root: bool, env_generics: Option<&'a Generics>, func: &'a mut ImplItemFn
	) -> Self {
		Self {
			is_root,
			attrs: &mut func.attrs,
			env_generics,
			sig: &mut func.sig,
			block: Some(&mut func.block)
		}
	}

	pub fn from_trait_fn(
		is_root: bool, env_generics: Option<&'a Generics>, func: &'a mut TraitItemFn
	) -> Self {
		Self {
			is_root,
			attrs: &mut func.attrs,
			env_generics,
			sig: &mut func.sig,
			block: func.default.as_mut()
		}
	}
}

#[derive(Clone)]
pub enum Functions {
	Fn(ImplItemFn),
	TraitFn(TraitItemFn),
	Trait(ItemTrait),
	Impl(ItemImpl)
}

impl ToTokens for Functions {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self {
			Self::Fn(func) => func.to_tokens(tokens),
			Self::TraitFn(func) => func.to_tokens(tokens),
			Self::Impl(item) => item.to_tokens(tokens),
			Self::Trait(item) => item.to_tokens(tokens)
		}
	}
}

impl Parse for Functions {
	fn parse(item: ParseStream<'_>) -> Result<Self> {
		let lookahead = item.fork();

		lookahead.call(Attribute::parse_outer)?;
		lookahead.parse::<Visibility>()?;
		lookahead.parse::<Option<Token![default]>>()?;
		lookahead.parse::<Option<Token![unsafe]>>()?;

		if lookahead.peek(Token![auto]) || lookahead.peek(Token![trait]) {
			return item.parse().map(Self::Trait);
		}

		if lookahead.peek(Token![impl]) {
			return item.parse().map(Self::Impl);
		}

		lookahead.parse::<Option<Token![const]>>()?;
		lookahead.parse::<Option<Token![async]>>()?;
		lookahead.parse::<Option<Token![unsafe]>>()?;
		lookahead.parse::<Option<Abi>>()?;

		if lookahead.parse::<Token![fn]>().is_err() {
			return Err(lookahead.error("Expected a function, trait, or impl"));
		}

		if let Ok(item) = item.parse() {
			return Ok(Self::Fn(item));
		}

		item.parse().map(Self::TraitFn)
	}
}

pub fn create_doc_item_fn(func: &ImplItemFn) -> ImplItemFn {
	let mut func = func.clone();

	func.attrs = vec![parse_quote! { #[cfg(doc)] }];
	func.block.stmts.clear();
	func
}

pub fn create_doc_trait_fn(func: &TraitItemFn) -> TraitItemFn {
	let mut func = func.clone();

	func.attrs = vec![parse_quote! { #[cfg(doc)] }];

	if let Some(block) = func.default.as_mut() {
		block.stmts.clear();
	}

	func
}

impl Functions {
	pub fn transform_all(
		self, callback: impl Fn(&mut Function<'_>) -> Result<()>,
		allowed: impl FnOnce(&Self) -> bool
	) -> Result<TokenStream> {
		if !allowed(&self) {
			return Err(Error::new_spanned(self, "Unexpected declaration"));
		}

		Ok(match self {
			Self::Fn(mut func) => {
				let mut original = create_doc_item_fn(&func);

				callback(&mut Function::from_item_fn(true, None, &mut func))?;

				original.attrs.extend_from_slice(&func.attrs);

				quote! {
					#original

					#[cfg(not(doc))]
					#func
				}
			}

			Self::TraitFn(mut func) => {
				let mut original = create_doc_trait_fn(&func);

				callback(&mut Function::from_trait_fn(true, None, &mut func))?;

				original.attrs.extend_from_slice(&func.attrs);

				quote! {
					#original

					#[cfg(not(doc))]
					#func
				}
			}

			Self::Impl(mut item) => {
				let mut originals = Vec::new();

				for impl_item in &mut item.items {
					let ImplItem::Fn(func) = impl_item else {
						continue;
					};

					let mut original = create_doc_item_fn(func);

					callback(&mut Function::from_item_fn(
						false,
						Some(&item.generics),
						func
					))?;

					original.attrs.extend_from_slice(&func.attrs);
					originals.push(ImplItem::Fn(original));
					func.attrs.push(parse_quote! { #[cfg(not(doc))] });
				}

				item.items.append(&mut originals);
				item.to_token_stream()
			}

			Self::Trait(mut item) => {
				let mut originals = Vec::new();

				for trait_item in &mut item.items {
					let TraitItem::Fn(func) = trait_item else {
						continue;
					};

					let mut original = create_doc_trait_fn(func);

					callback(&mut Function::from_trait_fn(
						false,
						Some(&item.generics),
						func
					))?;

					original.attrs.extend_from_slice(&func.attrs);
					originals.push(TraitItem::Fn(original));
					func.attrs.push(parse_quote! { #[cfg(not(doc))] });
				}

				item.items.append(&mut originals);
				item.to_token_stream()
			}
		})
	}
}

pub fn transform_fn(
	item: TokenStream, callback: impl Fn(&mut Function<'_>) -> Result<()>,
	allowed: impl FnOnce(&Functions) -> bool
) -> TokenStream {
	try_expand(|| {
		parse2::<Functions>(item).and_then(|parsed| parsed.transform_all(callback, allowed))
	})
}
