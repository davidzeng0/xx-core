use nom::number::complete::double;

use super::*;

type Result = std::result::Result<TokenStream, &'static str>;

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

fn parse_time_string(mut amount: &str) -> Result {
	let mut format = None;
	let mut tokens = Vec::new();

	while !amount.is_empty() {
		let lit = match double::<_, ()>(&amount as &str) {
			Ok((rest, lit)) => {
				amount = rest;
				lit
			}

			Err(_) => return Err("Expected a number literal")
		};

		if lit < 0.0 {
			return Err("Cannot be negative");
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
				return Err("Amount exceeds maximum");
			}

			current_format = Some(Format::Unit);
			scale = Some(
				scales
					.iter()
					.skip(index)
					.fold(1.0, |acc, value| acc * value)
			);
		} else {
			return Err("Unknown format");
		}

		if format == None {
			format = current_format;
		} else if format != current_format {
			return Err("Cannot use mismatched formats");
		}

		tokens.push((lit, scale));
	}

	let nanos = match format {
		None => return Err("Unknown format"),
		Some(Format::Unit) => {
			let mut duration = 0.0;

			for (lit, scale) in &tokens {
				duration += lit * scale.unwrap() as f64;
			}

			duration as u128
		}

		Some(Format::Colon) => {
			let mut duration = 0.0;
			let scales = [60.0, 60.0, 24.0];

			tokens.reverse();

			if tokens.len() > scales.len() + 1 {
				return Err("Too many tokens");
			}

			for (scale, (lit, _)) in scales.iter().zip(tokens.iter()) {
				if lit >= scale {
					return Err("Amount exceeds maximum");
				}
			}

			for i in 0..tokens.len() {
				let mut amount = tokens[i].0;

				for scale in &scales[0..i] {
					amount *= scale;
				}

				duration += amount;
			}

			(duration * 1_000_000_000.0) as u128
		}
	};

	Ok(quote! { #nanos })
}

fn parse_inverse(expr: TokenStream) -> Result {
	let Ok(Expr::Binary(binary)) = parse2(expr.clone()) else {
		return Err("Expected a binary op");
	};

	let BinOp::Div(_) = binary.op else {
		return Err("Expected a divide op");
	};

	let (left, right) = (&binary.left, &binary.right);

	Ok(quote! {
		(#left) as u128 * 1_000_000_000 / (#right) as u128
	})
}

pub fn duration(item: TokenStream) -> TokenStream {
	let amount = item.to_string().replace(" ", "");
	let amount = &amount as &str;

	let duration = if amount.contains("/") {
		parse_inverse(item.clone())
	} else {
		parse_time_string(amount)
	};

	match duration {
		Ok(duration) => quote! { ::std::time::Duration::from_nanos((#duration) as u64) },
		Err(err) => Error::new_spanned(item, err).to_compile_error()
	}
}
