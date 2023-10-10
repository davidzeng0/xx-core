use proc_macro::TokenStream;
use quote::quote;
use syn::{visit_mut::VisitMut, *};

use crate::{
	closure::{get_return_type, into_basic_closure, into_closure, make_tuple_type, ReplaceSelf},
	transform::transform_fn
};

fn transform_func(
	_: bool, attrs: &mut Vec<Attribute>, sig: &mut Signature, block: Option<&mut Block>
) -> Result<()> {
	attrs.push(parse_quote!( #[must_use] ));

	let return_type = get_return_type(&sig.output);
	let mut default_cancel_capture = vec![quote! { xx_core::task::RequestPtr<#return_type> }];

	if sig.inputs.iter().any(|arg| match arg {
		FnArg::Receiver(_) => true,
		FnArg::Typed(_) => false
	}) {
		default_cancel_capture.insert(0, quote! { &mut Self });
	}

	let default_cancel_capture = make_tuple_type(default_cancel_capture);

	let mut cancel_closure_type = quote! {
		xx_core::task::closure::CancelClosure<#default_cancel_capture>
	};

	let block = if let Some(block) = block {
		if let Some(stmt) = block.stmts.first_mut() {
			if let Stmt::Item(Item::Fn(func)) = stmt {
				if func.sig.ident == "cancel" {
					func.sig.inputs.push(parse_quote! {
						request: xx_core::task::RequestPtr<#return_type>
					});

					cancel_closure_type = into_closure(
						&mut func.attrs,
						&mut func.sig,
						Some(&mut func.block),
						vec![quote! { () }],
						vec![quote! { () }],
						quote! { xx_core::task::closure::CancelClosure },
						|capture| quote! { xx_core::task::closure::CancelClosure<#capture> }
					)?;

					let inputs = func.sig.inputs.clone();
					let output = func.sig.output.clone();
					let block = func.block.clone();

					*stmt = parse_quote! {
						let cancel = | #inputs | #output #block;
					};

					ReplaceSelf {}.visit_stmt_mut(stmt);
				}
			}
		}

		Some(block)
	} else {
		None
	};

	sig.output = parse_quote! {
		-> xx_core::task::Progress<#return_type, #cancel_closure_type>
	};

	into_basic_closure(
		attrs,
		sig,
		block,
		vec![quote! { request }],
		vec![quote! { xx_core::task::RequestPtr<#return_type> }],
		|_| quote! { xx_core::task::Progress<#return_type, #cancel_closure_type> },
		Some(|_| {
			(
				quote! { xx_core::task::Task<#return_type, #cancel_closure_type> },
				quote! { xx_core::task::closure::TaskClosureWrap }
			)
		})
	)?;

	Ok(())
}

/// ### Input
/// ```
/// #[sync_task]
/// fn add<'a>(&'a mut self, a: i32, b: i32) -> i32 {
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
/// fn add<'a>(&'a mut self, a: i32, b: i32) ->
/// 	TaskClosure<
/// 		(&'a mut Self, i32, i32),
/// 		Progress<i32,
/// 			CancelClosure<(&'a mut Self, extra: i32, RequestPtr<i32>)>
/// 		>
/// 	> {
/// 	let run = |
/// 		(__self, a, b): (&'a mut Self, i32, i32),
/// 		request: RequestPtr<i32>
/// 	| -> Progress<i32, CancelClosure<...>> {
/// 		let cancel = |
/// 			self: &'a mut Self, extra: i32
/// 		| {
/// 			let run = |
/// 				(__self, extra, request): (&'a mut Self, i32, RequestPtr<i32>)
/// 			| -> Result<()> {
/// 				self.cancel_async_add(extra, request)?;
///
/// 				Ok(())
/// 			};
///
/// 			CancelClosure::new((self, a + b, request), cancel)
/// 		}
///
/// 		if self.requires_async_add(a, b) {
/// 			self.async_add(a, b, request);
///
/// 			Progress::Pending(cancel(self, a + b, request))
/// 		} else {
/// 				Progress::Done(a + b)
/// 		}
/// 	};
///
/// 	TaskClosure::new((self, a, b), run)
/// }
/// ```
pub fn sync_task(_: TokenStream, item: TokenStream) -> TokenStream {
	match transform_fn(item, transform_func) {
		Ok(ts) => ts,
		Err(err) => err.to_compile_error().into()
	}
}
