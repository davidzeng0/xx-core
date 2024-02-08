use super::*;

struct SyscallImpl {
	instruction: LitStr,
	out: String,
	num: String,
	regs: Vec<String>,
	clobber: Vec<String>
}

impl Parse for SyscallImpl {
	fn parse(input: ParseStream) -> Result<Self> {
		let instruction: LitStr = input.parse::<LitStr>()?;

		input.parse::<Token![;]>()?;

		let mut out = None;
		let mut num = None;
		let mut args = None;
		let mut clobber = None;

		while !input.is_empty() {
			let kind = input.parse::<Ident>()?;

			input.parse::<Token![=]>()?;

			let mut regs = Vec::new();

			loop {
				regs.push(input.parse::<Ident>()?.to_string());

				if input.peek(Token![;]) {
					break;
				}

				input.parse::<Token![,]>()?;
			}

			input.parse::<Token![;]>()?;

			match kind.to_string().as_ref() {
				"out" => {
					if regs.len() != 1 {
						return Err(Error::new(kind.span(), "invalid output register list"));
					}

					out = Some(regs[0].clone());
				}

				"num" => {
					if regs.len() != 1 {
						return Err(Error::new(kind.span(), "invalid number register list"));
					}

					num = Some(regs[0].clone());
				}

				"arg" => args = Some(regs),
				"clobber" => clobber = Some(regs),

				_ => return Err(Error::new(kind.span(), "unknown kind"))
			}
		}

		Ok(Self {
			instruction,
			out: out.ok_or(Error::new(Span::call_site(), "expected output register"))?,
			num: num.ok_or(Error::new(Span::call_site(), "expected number register"))?,
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
		let args: Vec<_> = regs
			.iter()
			.enumerate()
			.map(|(i, _)| format_ident!("arg{}", i))
			.collect();

		for argc in 0..=regs.len() {
			let regs = &regs[0..argc];
			let args = &args[0..argc];
			let func = format_ident!("syscall{}", argc);

			functions.push(quote! {
				pub unsafe fn #func(num: i32, #(#args: impl Into<SyscallParameter>),*) -> isize {
					let result;

					::std::arch::asm!(
						#instruction,
						in(#num) num,
						#(in(#regs) #args.into().0,)*
						lateout(#out) result,
						#(lateout(#clobber) _),*
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
