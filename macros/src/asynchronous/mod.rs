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
	let mut lang = None;

	if let Some(attr) = remove_attr_name_value(attrs, "lang") {
		let Expr::Lit(ExprLit { lit: Lit::Str(str), .. }) = &attr.value else {
			return Err(Error::new_spanned(attr.value, "Expected a str"));
		};

		lang = Some((
			match str.value().as_ref() {
				"get_context" => Lang::GetContext,
				"task_wrap" => Lang::TaskWrap,
				"task_closure" => Lang::TaskClosure,
				"async_closure" => Lang::AsyncClosure,
				_ => return Err(Error::new_spanned(str, "Unknown lang item"))
			},
			attr.span()
		));
	}

	Ok(lang)
}

fn get_context_lifetime(attrs: &mut Vec<Attribute>) -> Result<Option<Lifetime>> {
	let mut lifetime = None;

	if let Some(attr) = remove_attr_list(attrs, "context") {
		let Ok(lt) = parse2(attr.tokens.clone()) else {
			return Err(Error::new_spanned(attr.tokens, "Expected a lifetime"));
		};

		lifetime = Some(lt);
	}

	Ok(lifetime)
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

fn parse_attrs(attrs: TokenStream) -> Result<AttributeArgs> {
	let mut parsed = AttributeArgs::new(AsyncKind::Default, attrs.span());

	let options = Punctuated::<Ident, Token![,]>::parse_terminated.parse2(attrs)?;

	for option in &options {
		if parsed.async_kind.0 != AsyncKind::Default {
			let message = "Invalid combination of options";

			return Err(Error::new_spanned(options, message));
		}

		let kind = AsyncKind::from_str(&option.to_string())
			.ok_or_else(|| Error::new_spanned(option, "Unknown option"))?;
		parsed.async_kind = (kind, option.span());
	}

	Ok(parsed)
}

fn language_impl(attrs: AttributeArgs, item: AsyncItem) -> Result<TokenStream> {
	let (lang, span) = attrs.language.unwrap();
	let use_lang = quote_spanned! { span =>
		#[allow(unused_imports)]
		use ::xx_core::coroutines::lang;
	};

	match (lang, item) {
		(Lang::TaskWrap, AsyncItem::Struct(item)) => Ok(task_lang_impl(use_lang, item, &[])),
		(Lang::TaskClosure, AsyncItem::Struct(item)) => Ok(task_lang_impl(
			use_lang,
			item,
			&[parse_quote! { #[inline(always)] }]
		)),
		(Lang::AsyncClosure, AsyncItem::Struct(item)) => Ok(async_closure_impl(use_lang, item)),
		_ => Err(Error::new(span, "Invalid language item"))
	}
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

	if attrs.async_kind.0 == AsyncKind::Task {
		if let AsyncItem::Impl(imp) = &mut item {
			/* hides the context pointer from the user, so this is safe */
			imp.unsafety = Some(Default::default());
		}
	}

	match &mut item {
		AsyncItem::Struct(item) => {
			attrs.parse(&mut item.attrs)?;
		}

		AsyncItem::Trait(item) => {
			attrs.parse(&mut item.attrs)?;
		}

		AsyncItem::Impl(imp) => {
			attrs.parse(&mut imp.attrs)?;
		}

		_ => ()
	}

	if let Some(lt) = &attrs.context_lifetime {
		return Err(Error::new_spanned(lt, "Context lifetime not allowed here"));
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
		let attrs = parse_attrs(attrs)?;

		try_transform(attrs, item)
	})
}
