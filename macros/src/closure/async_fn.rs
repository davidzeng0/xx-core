use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{spanned::Spanned, visit_mut::*, *};

use super::{make_closure::*, transform::*};

struct ReplaceAwait;

impl VisitMut for ReplaceAwait {
	fn visit_expr_mut(&mut self, expr: &mut Expr) {
		visit_expr_mut(self, expr);

		if let Expr::Await(inner) = expr {
			let base = inner.base.as_ref();
			let mut call: ExprCall = parse_quote_spanned! {
				Span::call_site() =>
					xx_core::coroutines::Context::run(
						__xx_internal_async_context.clone().as_mut(),
						#base
					)
			};

			call.attrs = inner.attrs.clone();

			*expr = Expr::Call(call);
		}
	}
}

#[derive(PartialEq, Eq)]
enum ClosureType {
	None   = 0,
	Opaque = 1,
	Typed  = 2
}

fn transform_with_type(
	is_item_fn: bool, attrs: &mut Vec<Attribute>, env_generics: Option<&Generics>,
	sig: &mut Signature, block: Option<&mut Block>, make_closure: ClosureType
) -> Result<()> {
	if sig.asyncness.take().is_none() {
		if !is_item_fn {
			return Ok(());
		}

		return Err(Error::new(
			sig.span(),
			"The `async` keyword is missing from the function declaration"
		));
	}

	if make_closure != ClosureType::None {
		attrs.push(parse_quote!( #[must_use = "Future does nothing until you `.await` it"] ));
	}

	let block = if let Some(block) = block {
		ReplaceAwait {}.visit_block_mut(block);
		Some(block)
	} else {
		None
	};

	match make_closure {
		ClosureType::None => {
			let mutability = if block.is_some() {
				quote! { mut }
			} else {
				quote! {}
			};

			sig.inputs.push(parse_quote! {
				#mutability __xx_internal_async_context:
					xx_core::task::Handle<xx_core::coroutines::Context>
			});
		}

		ClosureType::Opaque => {
			into_opaque_closure(
				attrs,
				&env_generics,
				sig,
				block,
				vec![quote! { mut __xx_internal_async_context }],
				vec![quote! { xx_core::task::Handle<xx_core::coroutines::Context> }],
				|rt| rt,
				Some(|rt| {
					(
						quote! { xx_core::coroutines::Task<Output = #rt> },
						quote! { xx_core::coroutines::ClosureWrap }
					)
				})
			)?;
		}

		ClosureType::Typed => {
			into_typed_closure(
				attrs,
				&env_generics,
				sig,
				block,
				vec![quote! { mut __xx_internal_async_context }],
				vec![quote! { xx_core::task::Handle<xx_core::coroutines::Context> }],
				quote! { xx_core::coroutines::Closure },
				|capture, ret| {
					quote! { xx_core::coroutines::Closure<#capture, #ret> }
				}
			)?;
		}
	}
	Ok(())
}

fn transform(
	is_item_fn: bool, attrs: &mut Vec<Attribute>, env_generics: Option<&Generics>,
	sig: &mut Signature, block: Option<&mut Block>
) -> Result<()> {
	transform_with_type(
		is_item_fn,
		attrs,
		env_generics,
		sig,
		block,
		ClosureType::Opaque
	)?;

	Ok(())
}

pub fn transform_no_closure(
	is_item_fn: bool, attrs: &mut Vec<Attribute>, env_generics: Option<&Generics>,
	sig: &mut Signature, block: Option<&mut Block>
) -> Result<()> {
	transform_with_type(
		is_item_fn,
		attrs,
		env_generics,
		sig,
		block,
		ClosureType::None
	)?;

	Ok(())
}

pub fn transform_typed_closure(
	is_item_fn: bool, attrs: &mut Vec<Attribute>, env_generics: Option<&Generics>,
	sig: &mut Signature, block: Option<&mut Block>
) -> Result<()> {
	transform_with_type(
		is_item_fn,
		attrs,
		env_generics,
		sig,
		block,
		ClosureType::Typed
	)?;

	Ok(())
}

/// ### Input
/// ```
/// #[async_fn]
/// async fn async_add(&mut self, a: i32, b: i32) -> i32 {
/// 	a + b
/// }
/// ```
///
/// ### Output
/// ```
/// fn async_add(&mut self, a: i32, b: i32) ->
/// 	AsyncClosure<(&mut Self, i32, i32), i32> {
/// 	let run = |
/// 		(__self, a, b): (&mut Self, i32, i32),
/// 		context: AsyncContext
/// 	| -> i32 {
/// 		a + b
/// 	}
///
/// 	AsyncClosure::new((self, a, b), run)
/// }
/// ```
pub fn async_fn(_: TokenStream, item: TokenStream) -> TokenStream {
	match transform_fn(item, transform) {
		Ok(ts) => ts,
		Err(err) => err.to_compile_error()
	}
}
