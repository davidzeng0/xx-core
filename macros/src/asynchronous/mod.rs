use std::mem::take;

use syn::parse::discouraged::Speculative;

use super::*;

pub mod branch;
mod invoke;
mod lang;
mod traits;
mod transform;

use invoke::*;
use lang::*;
use traits::*;
use transform::*;

fn get_lang(attrs: &mut Vec<Attribute>) -> Result<Option<(Lang, Span)>> {
	let Some(attr) = remove_attr_name_value(attrs, "lang") else {
		return Ok(None);
	};

	let Expr::Lit(ExprLit { lit: Lit::Str(str), .. }) = &attr.value else {
		return Err(Error::new_spanned(attr.value, "Expected a string"));
	};

	let lang = str
		.value()
		.parse()
		.map_err(|()| Error::new_spanned(str, "Unknown lang item"))?;
	Ok(Some((lang, attr.span())))
}

fn remove_attrs(attrs: &mut Vec<Attribute>, targets: &[&str]) -> Vec<Attribute> {
	let mut removed = Vec::new();

	for target in targets {
		while let Some(attr) = remove_attr_kind(attrs, target, |_| true) {
			removed.push(attr);
		}
	}

	removed
}

fn language_impl(mut attrs: AttributeArgs, item: AsyncItem) -> Result<TokenStream> {
	let (lang, span) = attrs.language.take().unwrap();
	let use_lang = quote_spanned! { span =>
		#[allow(unused_imports)]
		use ::xx_core::coroutines::lang;
	};

	Ok(match (lang, item) {
		(Lang::TaskWrap, AsyncItem::Struct(item)) => task_wrap_impl(use_lang, item, &[]),
		(Lang::TaskClosure, AsyncItem::Struct(item)) => {
			task_wrap_impl(use_lang, item, &[parse_quote! { #[inline(always)] }])
		}

		(Lang::AsyncClosure, AsyncItem::Struct(item)) => async_closure_impl(use_lang, item),
		(Lang::Task, AsyncItem::Trait(task)) => task_impl(attrs, use_lang, task)?,
		_ => return Err(Error::new(span, "Invalid language item"))
	})
}

fn async_items(item: Functions) -> Result<TokenStream> {
	item.transform_all(
		|func| {
			if let Some(block) = &mut func.block {
				TransformItems.visit_block_mut(block);
			}

			Ok(())
		},
		|_| true
	)
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
			(AsyncItem::Trait(_), AsyncKind::Default)
		) {
			return Err(Error::new(span, "Not allowed here"));
		}
	}

	if attrs.language.is_some() {
		return language_impl(attrs, item);
	}

	let mut item = match item {
		AsyncItem::Fn(item) => Functions::Fn(item),
		AsyncItem::TraitFn(item) => Functions::TraitFn(item),
		AsyncItem::Trait(item) => Functions::Trait(item),
		AsyncItem::Impl(item) => Functions::Impl(item),
		AsyncItem::Struct(item) => return Err(Error::new_spanned(item, "Unexpected declaration"))
	};

	#[allow(clippy::never_loop)]
	loop {
		if attrs.async_kind.0 != AsyncKind::Task {
			break;
		}

		let Functions::Impl(imp) = &mut item else {
			break;
		};

		let Some(ImplItem::Fn(run)) = imp.items.iter_mut().find(|item| match item {
			ImplItem::Fn(func) => func.sig.ident == "run",
			_ => false
		}) else {
			break;
		};

		if run.sig.unsafety.is_none() {
			/* caller must ensure we're allowed to suspend */
			run.sig.unsafety = Some(Default::default());
		}

		break;
	}

	let transform_functions = |attrs: AttributeArgs| {
		item.clone().transform_all(
			|func| transform_async(attrs.clone(), func),
			|item| {
				attrs.async_kind.0 == AsyncKind::Task ||
					!matches!(item, Functions::Trait(_) | Functions::TraitFn(_))
			}
		)
	};

	match attrs.async_kind.0 {
		AsyncKind::Default => (),
		AsyncKind::TraitFn => return async_impl(attrs, item),
		AsyncKind::Sync => return async_items(item),
		_ => return transform_functions(attrs)
	}

	match &item {
		Functions::Trait(item) => async_trait(attrs, item.clone()),
		Functions::Impl(imp) if imp.trait_.is_some() => async_impl(attrs, item.clone()),
		Functions::Fn(_) | Functions::Impl(_) => transform_functions(attrs),
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
