use std::mem::take;

use super::*;

pub mod branch;
mod invoke;
mod lang;
mod traits;
mod transform;

use self::invoke::*;
use self::lang::*;
use self::traits::*;
use self::transform::*;

#[allow(clippy::missing_panics_doc)]
fn language_impl(mut attrs: AttributeArgs, item: AsyncItem) -> Result<TokenStream> {
	let (lang, span) = attrs.language.take().unwrap();

	let use_lang = quote_spanned! { span =>
		#[allow(unused_imports)]
		use ::xx_core::coroutines::lang;
	};

	let inline = parse_quote! { #[inline(always)] };

	Ok(match (lang, item) {
		(Lang::TaskWrap, AsyncItem::Struct(item)) => task_wrap_impl(use_lang, item, &[]),
		(Lang::TaskClosure, AsyncItem::Struct(item)) => task_wrap_impl(use_lang, item, &[inline]),
		(Lang::AsyncClosure, AsyncItem::Struct(item)) => async_closure_impl(use_lang, item),
		(Lang::Task, AsyncItem::Trait(task)) => task_impl(attrs, use_lang, task)?,
		_ => return Err(Error::new(span, "Invalid language item"))
	})
}

fn try_transform(mut attrs: AttributeArgs, item: TokenStream) -> Result<TokenStream> {
	let mut item = parse2::<AsyncItem>(item)?;

	match &mut item {
		AsyncItem::Struct(item) => attrs.parse_additional(&mut item.attrs)?,
		AsyncItem::Trait(item) => attrs.parse_additional(&mut item.attrs)?,
		AsyncItem::Impl(imp) => attrs.parse_additional(&mut imp.attrs)?,
		_ => ()
	}

	if let Some(span) = attrs.impl_gen.span() {
		if !matches!(
			(&item, attrs.async_kind.0),
			(AsyncItem::Trait(_), AsyncKind::Default | AsyncKind::TraitFn)
		) {
			return Err(Error::new(span, "Not allowed here"));
		}
	}

	if attrs.language.is_some() {
		return language_impl(attrs, item);
	}

	let item = match item {
		AsyncItem::Fn(item) => Functions::Fn(item),
		AsyncItem::TraitFn(item) => Functions::TraitFn(item),
		AsyncItem::Trait(item) => Functions::Trait(item),
		AsyncItem::Impl(item) => Functions::Impl(item),
		AsyncItem::Struct(item) => return Err(Error::new_spanned(item, "Unexpected declaration"))
	};

	match attrs.async_kind.0 {
		AsyncKind::Default => (),
		AsyncKind::TraitFn => return async_impl(attrs, item),
		AsyncKind::Sync => return item.transform_all(None, transform_sync, |_| true),
		_ => return transform_items(item, attrs)
	}

	match &item {
		Functions::Trait(item) => async_trait(attrs, item.clone()),
		Functions::Impl(imp) if imp.trait_.is_some() => async_impl(attrs, item.clone()),
		Functions::Fn(_) | Functions::Impl(_) => transform_items(item, attrs),
		Functions::TraitFn(_) => {
			let message = "Trait functions must specify `#[asynchronous(traitfn)]`";

			Err(Error::new(Span::call_site(), message))
		}
	}
}

pub fn asynchronous(attrs: TokenStream, item: TokenStream) -> TokenStream {
	try_expand(|| {
		let attrs = AttributeArgs::parse.parse2(attrs)?;

		try_transform(attrs, item)
	})
}
