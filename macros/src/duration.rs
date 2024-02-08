use nom::number::complete::double;

use super::*;

#[derive(PartialEq, Eq)]
enum Format {
	Unit,
	Colon
}

fn test_prefix(input: &mut &str, prefixes: &[&str]) -> Option<usize> {
	let mut result = None;

	for (index, prefix) in prefixes.iter().enumerate() {
		if !input.starts_with(prefix) {
			continue;
		}

		if result.is_some_and(|(_, len)| len >= prefix.len()) {
			continue;
		}

		result = Some((index, prefix.len()));
	}

	result.map(|(index, len)| {
		*input = &input[len..];
		index
	})
}

fn parse_time_string(mut amount: &str, item: TokenStream) -> Result<TokenStream> {
	macro_rules! error {
		($message: literal) => {
			Err(Error::new(item.span(), $message))
		};
	}

	let mut format = None;
	let mut tokens = Vec::new();

	while !amount.is_empty() {
		let lit = match double::<_, ()>(&amount as &str) {
			Ok((rest, lit)) => {
				amount = rest;
				lit
			}

			Err(_) => {
				return error!("Expected a number literal");
			}
		};

		if lit < 0.0 {
			return error!("Cannot be negative");
		}

		if format == Some(Format::Colon) && amount.is_empty() {
			tokens.push((lit, None));

			break;
		}

		let mut scale = None;
		let current_format;

		if let Some(_) = test_prefix(&mut amount, &[":", "::"]) {
			current_format = Some(Format::Colon);
		} else if let Some(index) =
			test_prefix(&mut amount, &["d", "h", "m", "s", "ms", "us", "ns"])
		{
			let scales = [24.0, 60.0, 60.0, 1_000.0, 1_000.0, 1_000.0];

			if index > 0 && lit >= scales[index - 1] as f64 {
				return error!("Amount exceeds maximum");
			}

			current_format = Some(Format::Unit);
			scale = Some(
				scales
					.iter()
					.skip(index)
					.fold(1.0, |acc, value| acc * value)
			);
		} else {
			return error!("Unknown format");
		}

		if format == None {
			format = current_format;
		} else if format != current_format {
			return error!("Cannot use mismatched formats");
		}

		tokens.push((lit, scale));
	}

	let mut duration = 0.0;

	match format {
		None => return error!("Unknown format"),
		Some(Format::Unit) => {
			for (lit, scale) in &tokens {
				duration += lit * scale.unwrap() as f64;
			}

			duration /= 1_000_000_000.0;
		}

		Some(Format::Colon) => {
			let scales = [60.0, 60.0, 24.0];

			tokens.reverse();

			if tokens.len() > scales.len() + 1 {
				return error!("Too many tokens");
			}

			for (scale, (lit, _)) in scales.iter().zip(tokens.iter()) {
				if lit >= scale {
					return error!("Amount exceeds maximum");
				}
			}

			for i in 0..tokens.len() {
				let mut amount = tokens[i].0;

				for scale in &scales[0..i] {
					amount *= scale;
				}

				duration += amount;
			}
		}
	}

	Ok(quote! { #duration })
}

fn parse_inverse(expr: TokenStream) -> Result<TokenStream> {
	macro_rules! error {
		($message: literal) => {
			Err(Error::new(expr.span(), $message))
		};
	}

	let Expr::Binary(binary) = parse2(expr.clone())? else {
		return error!("Expected a binary op");
	};

	let BinOp::Div(_) = binary.op else {
		return error!("Expected a divide op");
	};

	let (left, right) = (&binary.left, &binary.right);

	Ok(quote! {
		(#left) as f64 / (#right) as f64
	})
}

pub fn duration(item: TokenStream) -> TokenStream {
	let amount = item.to_string().replace(" ", "");
	let amount = &amount as &str;

	let duration = if amount.contains("/") {
		parse_inverse(item)
	} else {
		parse_time_string(amount, item)
	};

	match duration {
		Ok(duration) => quote! { ::std::time::Duration::from_secs_f64(#duration) },
		Err(err) => err.to_compile_error()
	}
}
