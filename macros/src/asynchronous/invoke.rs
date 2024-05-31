use super::*;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClosureType {
	None,
	Standard,
	Trait
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum AsyncKind {
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

	pub fn from_str(str: &str) -> Option<Self> {
		Some(match str {
			"traitfn" => Self::TraitFn,
			"traitext" => Self::TraitExt,
			"task" => Self::Task,
			"sync" => Self::Sync,
			_ => return None
		})
	}
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lang {
	GetContext,
	TaskWrap,
	TaskClosure,
	AsyncClosure
}

#[derive(Clone)]
pub struct AttributeArgs {
	pub async_kind: (AsyncKind, Span),
	pub language: Option<(Lang, Span)>,
	pub context_lifetime: Option<Lifetime>
}

impl AttributeArgs {
	pub const fn new(async_kind: AsyncKind, span: Span) -> Self {
		Self {
			async_kind: (async_kind, span),
			language: None,
			context_lifetime: None
		}
	}

	pub fn parse(&mut self, attrs: &mut Vec<Attribute>) -> Result<()> {
		self.language = get_lang(attrs)?;
		self.context_lifetime = get_context_lifetime(attrs)?;

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
