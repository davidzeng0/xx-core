use proc_macro2::TokenStream;
use quote::quote;
use syn::{visit_mut::VisitMut, *};

use super::{make_closure::*, transform::*};

fn transform_func(func: &mut Function) -> Result<()> {
	func.attrs.push(parse_quote!( #[must_use] ));

	let return_type = get_return_type(&func.sig.output);
	let mut default_cancel_capture = vec![quote! { xx_core::task::RequestPtr<#return_type> }];

	if func.sig.inputs.iter().any(|arg| match arg {
		FnArg::Receiver(_) => true,
		FnArg::Typed(_) => false
	}) {
		default_cancel_capture.insert(0, quote! { &mut Self });
	}

	let default_cancel_capture = make_tuple_type(default_cancel_capture);

	let mut cancel_closure_type = quote! {
		xx_core::task::CancelClosure<#default_cancel_capture>
	};

	if let Some(block) = &mut func.block {
		loop {
			let Some(stmt) = (*block).stmts.first_mut() else {
				break;
			};

			let Stmt::Item(Item::Fn(cancel)) = stmt else {
				break;
			};

			if cancel.sig.ident != "cancel" {
				break;
			}

			cancel.sig.inputs.push(parse_quote! {
				request: xx_core::task::RequestPtr<#return_type>
			});

			cancel_closure_type = into_typed_closure(
				&mut Function {
					is_item_fn: true,
					attrs: &mut cancel.attrs,
					env_generics: func.env_generics,
					sig: &mut cancel.sig,
					block: Some(&mut cancel.block)
				},
				vec![(quote! { () }, quote! { () })],
				quote! { xx_core::task::CancelClosure },
				|capture, _| quote! { xx_core::task::CancelClosure<#capture> }
			)?;

			let inputs = cancel.sig.inputs.clone();
			let output = cancel.sig.output.clone();
			let block = cancel.block.clone();

			*stmt = parse_quote! {
				let cancel = | #inputs | #output #block;
			};

			ReplaceSelf {}.visit_stmt_mut(stmt);
		}
	}

	func.sig.output = parse_quote! {
		-> xx_core::task::Progress<#return_type, #cancel_closure_type>
	};

	into_opaque_closure(
		func,
		vec![(
			quote! { request },
			quote! { xx_core::task::RequestPtr<#return_type> }
		)],
		|_| quote! { xx_core::task::Progress<#return_type, #cancel_closure_type> },
		OpaqueClosureType::Custom(|_| {
			(
				quote! { xx_core::task::Task<Output = #return_type, Cancel = #cancel_closure_type> },
				quote! { xx_core::task::TaskClosureWrap }
			)
		})
	)?;

	Ok(())
}

/// ### Input
/// ```
/// #[sync_task]
/// fn add(&mut self, a: i32, b: i32) -> i32 {
/// 	fn cancel(self: &'a mut Self, extra: i32) -> Result<()> {
/// 		self.cancel_async_add(extra, request)?;
///
/// 		Ok(())
/// 	}
///
/// 	if self.requires_async_add(a, b) {
/// 		self.async_add(a, b, request);
///
/// 		Progress::pending(cancel(self, a + b, request))
/// 	} else {
/// 		Progress::Done(a + b)
/// 	}
/// }
/// ```
///
/// ### Output
/// ```
/// #[must_use]
/// fn add<'closure, 'life1>(
/// 	&'life1 mut self, a: i32, b: i32
/// ) -> impl xx_core::task::Task<
/// 	Output = i32,
/// 	Cancel = xx_core::task::CancelClosure<(
/// 		&'life1 mut Self,
/// 		i32,
/// 		xx_core::task::RequestPtr<i32>
/// 	)>
/// > + 'closure
/// where
/// 	'life1: 'closure
/// {
/// 	xx_core::task::TaskClosureWrap::new(
/// 		move |request: xx_core::task::RequestPtr<i32>| -> xx_core::task::Progress<
/// 			i32,
/// 			xx_core::task::CancelClosure<(
/// 				&'life1 mut Self,
/// 				i32,
/// 				xx_core::task::RequestPtr<i32>
/// 			)>
/// 		> {
/// 			let cancel = |__self: &'life1 mut Self,
/// 			              extra: i32,
/// 			              request: xx_core::task::RequestPtr<i32>|
/// 			 -> xx_core::task::CancelClosure<(
/// 				&'life1 mut Self,
/// 				i32,
/// 				xx_core::task::RequestPtr<i32>
/// 			)> {
/// 				let run = |(__self, extra, request): (
/// 					&'life1 mut Self,
/// 					i32,
/// 					xx_core::task::RequestPtr<i32>
/// 				),
/// 				           (): ()|
/// 				 -> Result<()> {
/// 					__self.cancel_async_add(extra, request)?;
/// 					Ok(())
/// 				};
/// 				xx_core::task::CancelClosure::new((__self, extra, request), run)
/// 			};
///
/// 			if self.requires_async_add(a, b) {
/// 				self.async_add(a, b, request);
/// 				Progress::Pending(cancel(self, a + b, request))
/// 			} else {
/// 				Progress::Done(a + b)
/// 			}
/// 		}
/// 	)
/// }
/// ```
pub fn sync_task(_: TokenStream, item: TokenStream) -> TokenStream {
	match transform_fn(item, transform_func) {
		Ok(ts) => ts,
		Err(err) => err.to_compile_error()
	}
}
