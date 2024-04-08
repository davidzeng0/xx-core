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

			match kind.to_string().as_ref() {
				"out" => {
					if regs.len() != 1 {
						return Err(Error::new_spanned(
							kind,
							"There can only be one output register"
						));
					}

					out = Some(regs[0].clone());
				}

				"num" => {
					if regs.len() != 1 {
						return Err(Error::new_spanned(
							kind,
							"There can only be one number register"
						));
					}

					num = Some(regs[0].clone());
				}

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
	let syscall_impl = match parse2::<SyscallImpl>(item) {
		Ok(functions) => functions,
		Err(err) => return err.to_compile_error()
	};

	syscall_impl.expand()
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

			pat_ty.ty = parse_quote_spanned! { ty.span() =>
				<#ty as ::xx_core::os::syscall::IntoRaw>::Raw
			};

			into_raw.push(parse_quote_spanned! { pat.span() =>
				::xx_core::os::syscall::IntoRaw::into_raw(#pat)
			});

			raw_args.push(FnArg::Typed(pat_ty));

			continue;
		};

		let Pat::Ident(mut pat_ident) = *pat_ty.pat else {
			return Err(Error::new_spanned(pat_ty.pat, "Pattern not allowed here"));
		};

		let options = parse_options(&array.meta)?;
		let length_type = options.len;
		let into_length_type = length_type.as_ref().map(|len| {
			quote_spanned! { len.span() => .try_into().unwrap() }
		});

		vars.push(parse_quote_spanned! { pat_ident.span() =>
			let #pat_ident = ::xx_core::os::syscall::IntoRawArray::into_raw_array(#pat_ident);
		});

		into_raw.push(parse_quote! { (#pat_ident).0 });
		into_raw.push(parse_quote! { (#pat_ident).1#into_length_type });

		let ty = pat_ty.ty.clone();
		let ident = pat_ident.ident.clone();

		pat_ident.ident = format_ident!("{}_ptr", ident);
		pat_ty.pat = Box::new(Pat::Ident(pat_ident.clone()));
		pat_ty.ty = Box::new(parse_quote_spanned! { ty.span() =>
			<#ty as ::xx_core::os::syscall::IntoRawArray>::Pointer
		});

		raw_args.push(FnArg::Typed(pat_ty.clone()));

		pat_ident.ident = format_ident!("{}_len", ident);
		pat_ty.pat = Box::new(Pat::Ident(pat_ident));

		if let Some(expr) = &length_type {
			pat_ty.ty = Box::new(parse_quote! { #expr });
		} else {
			pat_ty.ty = Box::new(parse_quote_spanned! { ty.span() =>
				<#ty as ::xx_core::os::syscall::IntoRawArray>::Length
			});
		}

		raw_args.push(FnArg::Typed(pat_ty));
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
			let result = unsafe {
				::xx_core::os::syscall::syscall_raw!(
					(#number) as i32
					#(
						,
						::xx_core::os::syscall::SyscallParameter::from(#args).0
					)*
				)
			};

			::std::convert::From::<
				::xx_core::os::syscall::SyscallResult
			>::from(
				::xx_core::os::syscall::SyscallResult(result)
			)
		}

		#(#attrs)* #vis #sig {
			#(#vars)*

			unsafe { #raw_ident(#into_raw) }
		}
	})
}

pub fn syscall_define(attrs: TokenStream, item: TokenStream) -> TokenStream {
	match expand_syscall_define(attrs, item) {
		Ok(tokens) => tokens,
		Err(err) => err.to_compile_error()
	}
}
