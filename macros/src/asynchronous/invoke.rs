use super::*;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClosureType {
	None,
	Standard,
	Trait
}

#[strings(defaults, lowercase)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum AsyncKind {
	#[omit]
	Default,
	TraitFn,
	TraitExt,
	Task,
	Sync
}

impl AsyncKind {
	#[must_use]
	pub const fn closure_type(self) -> ClosureType {
		match self {
			Self::Default => ClosureType::Standard,
			Self::TraitFn => ClosureType::None,
			Self::TraitExt => ClosureType::Trait,
			Self::Task => ClosureType::None,
			Self::Sync => ClosureType::None
		}
	}
}

#[strings(defaults, snake)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lang {
	GetContext,
	TaskWrap,
	TaskClosure,
	AsyncClosure,
	Task
}

#[derive(Default, Clone)]
pub struct ImplGen {
	pub impl_ref: Option<Span>,
	pub impl_mut: Option<Span>,
	pub impl_box: Option<Span>
}

struct Ident(proc_macro2::Ident);

impl Parse for Ident {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let ident = input.step(|cursor| {
			if let Some(ident) = cursor.ident() {
				Ok(ident)
			} else {
				Err(cursor.error("Expected an identifier"))
			}
		})?;

		Ok(Self(ident))
	}
}

impl ToTokens for Ident {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.0.to_tokens(tokens);
	}
}

impl ImplGen {
	fn parse(&mut self, input: ParseStream<'_>) -> Result<()> {
		let options = Punctuated::<Ident, Token![,]>::parse_terminated(input)?;

		for option in options {
			let span = match option.0.to_string().as_ref() {
				"ref" => &mut self.impl_ref,
				"mut" => &mut self.impl_mut,
				"box" => &mut self.impl_box,
				_ => {
					let message = "Unknown option";

					return Err(Error::new_spanned(option, message));
				}
			};

			if span.is_some() {
				let message = "Duplicate option";

				return Err(Error::new_spanned(option, message));
			}

			*span = Some(option.span());
		}

		Ok(())
	}

	pub fn span(&self) -> Option<Span> {
		self.impl_mut.or(self.impl_box)
	}
}

#[derive(Clone)]
pub struct AttributeArgs {
	pub async_kind: (AsyncKind, Span),
	pub impl_gen: ImplGen,
	pub language: Option<(Lang, Span)>
}

impl AttributeArgs {
	pub fn new(async_kind: AsyncKind, span: Span) -> Self {
		Self {
			async_kind: (async_kind, span),
			language: None,
			impl_gen: ImplGen::default()
		}
	}

	pub fn parse(input: ParseStream<'_>) -> Result<Self> {
		let mut this = Self::new(AsyncKind::Default, input.span());

		while !input.is_empty() {
			let option = input.parse::<Ident>()?;
			let name = option.0.to_string();

			match name.as_ref() {
				"impl" => {
					let content;

					parenthesized!(content in input);

					this.impl_gen.parse(&content)?;
				}

				_ => {
					let kind = name
						.parse()
						.map_err(|()| Error::new_spanned(&option, "Unknown option"))?;

					if this.async_kind.0 != AsyncKind::Default {
						let message = "Invalid combination of options";

						return Err(Error::new_spanned(option, message));
					}

					this.async_kind = (kind, option.span());
				}
			}
		}

		Ok(this)
	}

	pub fn parse_additional(&mut self, attrs: &mut Vec<Attribute>) -> Result<()> {
		self.language = get_lang(attrs)?;

		Ok(())
	}
}

#[derive(Clone)]
pub enum AsyncItem {
	Fn(ImplItemFn),
	TraitFn(TraitItemFn),
	Trait(ItemTrait),
	Impl(ItemImpl),
	Struct(ItemStruct)
}

impl Parse for AsyncItem {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let fork = input.fork();

		if let Ok(item) = ItemStruct::parse(&fork) {
			input.advance_to(&fork);

			return Ok(Self::Struct(item));
		}

		Ok(match Functions::parse(input)? {
			Functions::Fn(item) => Self::Fn(item),
			Functions::TraitFn(item) => Self::TraitFn(item),
			Functions::Trait(item) => Self::Trait(item),
			Functions::Impl(item) => Self::Impl(item)
		})
	}
}
