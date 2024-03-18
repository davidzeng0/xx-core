use nom::{
	branch::alt,
	bytes::complete::{tag, tag_no_case},
	character::complete::multispace0,
	error::{ErrorKind, ParseError},
	multi::{many1, separated_list1},
	number::complete::double,
	sequence::tuple,
	IResult, Parser
};

use super::*;

type Result<T> = std::result::Result<T, &'static str>;

fn alt_vec<I: Clone, O, E, P>(mut choices: Vec<P>) -> impl FnMut(I) -> IResult<I, O, E>
where
	P: Parser<I, O, E>
{
	if choices.is_empty() {
		panic!();
	}

	move |input| {
		let mut error = None;

		for choice in &mut choices {
			match choice.parse(input.clone()) {
				Err(err @ nom::Err::Error(_)) => error = Some(err),
				res => return res
			}
		}

		Err(error.unwrap())
	}
}

fn parse_named_units<'a, E>(input: &'a str) -> IResult<&'a str, f64, E>
where
	E: ParseError<&'a str>
{
	let scales = [
		("d", 24.0),
		("h", 60.0),
		("m", 60.0),
		("s", 1000.0),
		("ms", 1000.0),
		("us", 1000.0),
		("ns", 1.0)
	];

	let mut tags = scales;

	tags.sort_by(|a, b| b.0.cmp(a.0));

	let tags = tags
		.iter()
		.map(|scale| {
			tuple((
				multispace0,
				double,
				multispace0,
				tag_no_case(scale.0),
				multispace0
			))
		})
		.collect();

	let mut parser = many1(alt_vec(tags));
	let (leftover, parsed) = parser(input)?;

	let mut duration = 0.0;

	for (_, amount, _, unit, _) in parsed {
		let index = scales.iter().position(|scale| scale.0 == unit).unwrap();
		let scale = scales[index].1;

		if amount < 0.0 || (amount >= scale && scale != 1.0) {
			return Err(nom::Err::Failure(E::from_error_kind(
				input,
				ErrorKind::Float
			)));
		}

		duration += amount * scales[index..].iter().fold(1.0, |acc, value| acc * value.1);
	}

	Ok((leftover, duration))
}

fn parse_unnamed_units<'a, E>(input: &'a str) -> IResult<&'a str, f64, E>
where
	E: ParseError<&'a str>
{
	let scales = [24.0, 60.0, 60.0, 1_000_000_000.0];

	let mut parser = separated_list1(
		tuple((multispace0, alt((tag("::"), tag(":"))), multispace0)),
		double
	);

	let (leftover, parsed) = parser(input)?;

	let mut duration = 0.0;

	if parsed.len() > scales.len() {
		return Err(nom::Err::Failure(E::from_error_kind(
			input,
			ErrorKind::SeparatedNonEmptyList
		)));
	}

	for (index, &amount) in parsed.iter().rev().enumerate() {
		let index = scales.len() - index - 1;
		let scale = scales[index];

		if amount < 0.0 || amount >= scale {
			return Err(nom::Err::Failure(E::from_error_kind(
				input,
				ErrorKind::Float
			)));
		}

		duration += amount * scales[index..].iter().fold(1.0, |acc, value| acc * value);
	}

	Ok((leftover, duration))
}

fn parse_time_string(amount: &str) -> Result<TokenStream> {
	let mut parser = alt::<_, _, (), _>((parse_named_units, parse_unnamed_units));
	let (leftover, nanos) = parser(amount).map_err(|_| "Unknown format")?;

	if !leftover.is_empty() {
		return Err("Unknown format (found trailing data)");
	}

	Ok(quote! { #nanos })
}

fn parse_inverse(expr: Expr) -> Result<TokenStream> {
	let Expr::Binary(binary) = expr else {
		return Err("Expected a binary op");
	};

	let BinOp::Div(_) = binary.op else {
		return Err("Expected a divide op");
	};

	let (left, right) = (&binary.left, &binary.right);

	Ok(quote! {{
		let (left, right) = (#left, #right);

		left as u128 * 1_000_000_000 / right as u128
	}})
}

pub fn duration(item: TokenStream) -> TokenStream {
	let amount = item.to_string();

	let duration = if let Ok(expr) = parse2(item.clone()) {
		parse_inverse(expr)
	} else {
		parse_time_string(amount.as_ref())
	};

	match duration {
		Ok(duration) => quote! { ::std::time::Duration::from_nanos((#duration) as u64) },
		Err(err) => Error::new_spanned(item, err).to_compile_error()
	}
}
