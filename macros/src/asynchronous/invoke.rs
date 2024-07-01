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

#[derive(Clone)]
pub struct AttributeArgs {
	pub async_kind: (AsyncKind, Span),
	pub language: Option<(Lang, Span)>
}

impl AttributeArgs {
	pub const fn new(async_kind: AsyncKind, span: Span) -> Self {
		Self { async_kind: (async_kind, span), language: None }
	}

	pub fn parse(args: TokenStream) -> Result<Self> {
		let mut this = Self::new(AsyncKind::Default, args.span());
		let options = Punctuated::<Ident, Token![,]>::parse_terminated.parse2(args)?;

		for option in &options {
			if this.async_kind.0 != AsyncKind::Default {
				let message = "Invalid combination of options";

				return Err(Error::new_spanned(options, message));
			}

			let kind = option
				.to_string()
				.parse()
				.map_err(|()| Error::new_spanned(option, "Unknown option"))?;
			this.async_kind = (kind, option.span());
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
