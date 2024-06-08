use super::*;

struct SyscallImpl {
	instruction: LitStr,
	out: String,
	num: String,
	regs: Vec<String>,
	clobber: Vec<String>
}

impl Parse for SyscallImpl {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let instruction: LitStr = input.parse::<LitStr>()?;

		input.parse::<Token![;]>()?;

		let mut out = None;
		let mut num = None;
		let mut args = None;
		let mut clobber = None;

		while !input.is_empty() {
			let kind = input.parse::<Ident>()?;

			input.parse::<Token![=]>()?;

			let regs = Punctuated::<Ident, Token![,]>::parse_separated_nonempty(input)?;
			let regs: Vec<_> = regs.iter().map(ToString::to_string).collect();

			input.parse::<Token![;]>()?;

			let kind_str = kind.to_string();

			match kind_str.as_ref() {
				kind @ ("out" | "num") if regs.len() != 1 => {
					let msg = format!("There can only be one `{}` register", kind);

					return Err(Error::new_spanned(kind, msg));
				}

				_ => ()
			}

			match kind.to_string().as_ref() {
				"out" => out = Some(regs[0].clone()),
				"num" => num = Some(regs[0].clone()),
				"arg" => args = Some(regs),
				"clobber" => clobber = Some(regs),

				_ => {
					return Err(Error::new_spanned(
						kind,
						"Unknown kind, expected one of `out`, `num`, `arg`, or `clobber`"
					))
				}
			}
		}

		Ok(Self {
			instruction,
			out: out.ok_or_else(|| input.error("Expected an output register `out = reg`"))?,
			num: num.ok_or_else(|| input.error("Expected a number register `num = reg`"))?,
			regs: args.unwrap_or(Vec::new()),
			clobber: clobber.unwrap_or(Vec::new())
		})
	}
}

impl SyscallImpl {
	fn expand(&self) -> TokenStream {
		let (instruction, out, num, regs, clobber) = (
			&self.instruction,
			&self.out,
			&self.num,
			&self.regs,
			&self.clobber
		);

		let mut functions = Vec::new();
		let args: Vec<_> = (0..regs.len()).map(|i| format_ident!("arg{}", i)).collect();

		for argc in 0..=regs.len() {
			let regs = &regs[0..argc];
			let args = &args[0..argc];
			let func = format_ident!("syscall{}", argc);

			functions.push(quote! {
				#[inline(always)]
				pub unsafe fn #func(num: i32, #(#args: usize),*) -> isize {
					let result;

					::std::arch::asm!(
						#instruction,
						in(#num) num,
						#(in(#regs) #args,)*
						lateout(#out) result,
						#(lateout(#clobber) _,)*
						options(nostack, preserves_flags)
					);

					result
				}
			});
		}

		quote! { #(#functions)* }
	}
}

pub fn syscall_impl(item: TokenStream) -> TokenStream {
	try_expand(|| parse2::<SyscallImpl>(item).map(|syscall| syscall.expand()))
}

struct ArrayOptions {
	len: Option<Expr>
}

fn parse_options(meta: &Meta) -> Result<ArrayOptions> {
	let mut options = ArrayOptions { len: None };

	let Meta::List(list) = meta else {
		return Ok(options);
	};

	let metas = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())?;

	for meta in metas {
		let Meta::NameValue(nv) = meta else {
			return Err(Error::new_spanned(
				meta,
				"Expected a `option` = `value` arg"
			));
		};

		if nv.path.get_ident().is_some_and(|ident| ident != "len") {
			return Err(Error::new_spanned(nv.path, "Unknown option"));
		}

		options.len = Some(nv.value);
	}

	Ok(options)
}

fn make_pat_type(pat_ty: &PatType, new_ident: Ident, ty: impl FnOnce(Type) -> Type) -> PatType {
	let mut result = pat_ty.clone();

	let Pat::Ident(ident) = result.pat.as_mut() else {
		unreachable!();
	};

	ident.ident = new_ident;
	result.ty = Box::new(ty(*result.ty));
	result
}

#[allow(clippy::type_complexity)]
fn get_raw_args(
	args: &mut Punctuated<FnArg, Token![,]>
) -> Result<(
	Punctuated<FnArg, Token![,]>,
	Vec<Stmt>,
	Punctuated<Expr, Token![,]>
)> {
	let (mut raw_args, mut vars, mut into_raw) = (Punctuated::new(), Vec::new(), Punctuated::new());

	for arg in args {
		let FnArg::Typed(ty) = arg else {
			return Err(Error::new_spanned(arg, "Receiver is not allowed here"));
		};

		let array = remove_attr_kind(&mut ty.attrs, "array", |meta| {
			matches!(meta, Meta::Path(_) | Meta::List(_))
		});

		let mut pat_ty = ty.clone();

		let Some(array) = array else {
			let (pat, ty) = (&pat_ty.pat, &pat_ty.ty);

			into_raw.push(parse_quote_spanned! { pat.span() =>
				IntoRaw::into_raw(#pat)
			});

			pat_ty.ty = parse_quote_spanned! { ty.span() =>
				<#ty as ::xx_core::os::syscall::IntoRaw>::Raw
			};

			raw_args.push(FnArg::Typed(pat_ty));

			continue;
		};

		let Pat::Ident(pat_ident) = pat_ty.pat.as_ref() else {
			return Err(Error::new_spanned(pat_ty.pat, "Pattern not allowed here"));
		};

		let ptr = make_pat_type(&pat_ty, format_ident!("{}_ptr", pat_ident.ident), |ty| {
			parse_quote_spanned! { ty.span() =>
				<#ty as ::xx_core::os::syscall::IntoRawArray>::Pointer
			}
		});

		let mut len = make_pat_type(&pat_ty, format_ident!("{}_len", pat_ident.ident), |ty| {
			parse_quote_spanned! { ty.span() =>
				<#ty as ::xx_core::os::syscall::IntoRawArray>::Length
			}
		});

		let options = parse_options(&array.meta)?;
		let length_type = options.len;
		let into_length_type = length_type.as_ref().map(|len| {
			quote_spanned! { len.span() => .try_into().unwrap() }
		});

		into_raw.push(parse_quote! { (#pat_ident).0 });
		into_raw.push(parse_quote! { (#pat_ident).1#into_length_type });

		vars.push(parse_quote_spanned! { pat_ident.span() =>
			let #pat_ident = IntoRawArray::into_raw_array(#pat_ident);
		});

		if let Some(expr) = &length_type {
			len.ty = Box::new(parse_quote! { #expr });
		}

		raw_args.push(FnArg::Typed(ptr));
		raw_args.push(FnArg::Typed(len));
	}

	Ok((raw_args, vars, into_raw))
}

fn expand_syscall_define(attrs: TokenStream, item: TokenStream) -> Result<TokenStream> {
	let number: Expr = parse2(attrs)?;
	let func: ForeignItemFn = parse2(item)?;

	let (attrs, vis, mut sig) = (&func.attrs, &func.vis, func.sig.clone());
	let (raw_args, vars, into_raw) = get_raw_args(&mut sig.inputs)?;
	let args: Vec<_> = get_args(&raw_args, false).into_iter().collect();

	let mut raw_sig = sig.clone();
	let raw_ident = format_ident!("{}_raw", raw_sig.ident);

	raw_sig.ident = raw_ident.clone();
	raw_sig.unsafety.get_or_insert(Default::default());
	raw_sig.inputs = raw_args;

	Ok(quote! {
		#(#attrs)* #vis #raw_sig {
			let number = (#number) as i32;

			{
				use ::std::convert::Into;
				use ::xx_core::os::syscall::{IntoRaw, IntoRawArray, SyscallParameter, syscall_raw};

				let result = unsafe {
					syscall_raw!(
						number
						#(, SyscallParameter::from(#args).0)*
					)
				};

				Into::into(SyscallResult(result))
			}
		}

		#(#attrs)* #vis #sig {
			use ::std::convert::TryInto;
			use ::xx_core::os::syscall::{IntoRaw, IntoRawArray};

			#(#vars)*

			unsafe { #raw_ident(#into_raw) }
		}
	})
}

pub fn syscall_define(attrs: TokenStream, item: TokenStream) -> TokenStream {
	try_expand(|| expand_syscall_define(attrs, item))
}
