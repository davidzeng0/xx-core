use super::*;

struct ReplaceAwait;

impl VisitMut for ReplaceAwait {
	fn visit_expr_mut(&mut self, expr: &mut Expr) {
		visit_expr_mut(self, expr);

		if let Expr::Await(inner) = expr {
			let base = inner.base.as_ref();

			*expr = Expr::Call(ExprCall {
				attrs: inner.attrs.clone(),
				func: parse_quote! { xx_core::coroutines::Context::run },
				paren_token: Default::default(),
				args: parse_quote! { __xx_internal_async_context.clone().as_mut(), #base }
			});
		}
	}
}

#[derive(PartialEq, Eq)]
enum ClosureType {
	None   = 0,
	Opaque = 1,
	Typed  = 2
}

fn transform_with_type(func: &mut Function, make_closure: ClosureType) -> Result<()> {
	if func.sig.asyncness.take().is_none() {
		if !func.is_item_fn {
			return Ok(());
		}

		return Err(Error::new(
			func.sig.span(),
			"The `async` keyword is missing from the function declaration"
		));
	}

	if make_closure != ClosureType::None {
		func.attrs
			.push(parse_quote!( #[must_use = "Future does nothing until you `.await` it"] ));
	}

	if let Some(block) = &mut func.block {
		ReplaceAwait {}.visit_block_mut(*block);
	}

	match make_closure {
		ClosureType::None => {
			let mut arg = parse_quote! {
				mut __xx_internal_async_context: xx_core::task::Handle<xx_core::coroutines::Context>
			};

			if func.block.is_none() {
				RemoveRefMut {}.visit_fn_arg_mut(&mut arg);
			}

			func.sig.inputs.push(arg);
		}

		ClosureType::Opaque => {
			into_opaque_closure(
				func,
				vec![(
					quote! { mut __xx_internal_async_context },
					quote! { xx_core::task::Handle<xx_core::coroutines::Context> }
				)],
				|rt| rt,
				OpaqueClosureType::Custom(|rt| {
					(
						quote! { xx_core::coroutines::Task<Output = #rt> },
						quote! { xx_core::coroutines::closure::ClosureWrap }
					)
				})
			)?;
		}

		ClosureType::Typed => {
			into_typed_closure(
				func,
				vec![(
					quote! { mut __xx_internal_async_context },
					quote! { xx_core::task::Handle<xx_core::coroutines::Context> }
				)],
				quote! { xx_core::coroutines::closure::Closure },
				|capture, ret| {
					quote! { xx_core::coroutines::closure::Closure<#capture, #ret> }
				}
			)?;
		}
	}

	Ok(())
}

fn transform(func: &mut Function) -> Result<()> {
	transform_with_type(func, ClosureType::Opaque)?;

	Ok(())
}

pub fn transform_no_closure(func: &mut Function) -> Result<()> {
	transform_with_type(func, ClosureType::None)?;

	Ok(())
}

pub fn transform_typed_closure(func: &mut Function) -> Result<()> {
	transform_with_type(func, ClosureType::Typed)?;

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
/// #[must_use = "Future does nothing until you `.await` it"]
/// fn async_add<'closure, 'life1>(
/// 	&'life1 mut self, a: i32, b: i32
/// ) -> impl xx_core::coroutines::Task<Output = i32> + 'closure
/// where
/// 	'life1: 'closure
/// {
/// 	xx_core::coroutines::ClosureWrap::new(
/// 		move |mut __xx_internal_async_context: xx_core::task::Handle<
/// 			xx_core::coroutines::Context
/// 		>|
/// 		      -> i32 { a + b }
/// 	)
/// }
/// ```
pub fn async_fn(_: TokenStream, item: TokenStream) -> TokenStream {
	match transform_fn(item, transform) {
		Ok(ts) => ts,
		Err(err) => err.to_compile_error()
	}
}

pub fn async_fn_typed(_: TokenStream, item: TokenStream) -> TokenStream {
	match transform_fn(item, transform_typed_closure) {
		Ok(ts) => ts,
		Err(err) => err.to_compile_error()
	}
}
