use proc_macro::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, visit_mut::*, *};

use crate::{closure::into_basic_closure, transform::transform_fn};

struct ReplaceAwait;

impl VisitMut for ReplaceAwait {
	fn visit_expr_mut(&mut self, expr: &mut Expr) {
		visit_expr_mut(self, expr);

		if let Expr::Await(inner) = expr {
			let base = inner.base.as_ref();
			let mut call: ExprCall = parse_quote! {
				xx_core::coroutines::env::AsyncContext::run(__xx_async_internal_context.clone().as_ref_mut(), #base)
			};

			call.attrs = inner.attrs.clone();

			*expr = Expr::Call(call);
		}
	}
}

fn transform_func(
	is_item_fn: bool, attrs: &mut Vec<Attribute>, sig: &mut Signature, block: Option<&mut Block>
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

	attrs.push(parse_quote!( #[must_use = "Future does nothing until you `.await` it"] ));

	let block = if let Some(block) = block {
		ReplaceAwait {}.visit_block_mut(block);
		Some(block)
	} else {
		None
	};

	into_basic_closure(
		attrs,
		sig,
		block,
		vec![quote! { mut __xx_async_internal_context }],
		vec![quote! { xx_core::task::env::Handle<xx_async_runtime::Context> }],
		|rt| rt,
		Some(|rt| {
			(
				quote! { xx_core::coroutines::task::AsyncTask<xx_async_runtime::Context, #rt> },
				quote! { xx_core::coroutines::closure::AsyncClosure }
			)
		})
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
	match transform_fn(item, transform_func) {
		Ok(ts) => ts,
		Err(err) => err.to_compile_error().into()
	}
}
