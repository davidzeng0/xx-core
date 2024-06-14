use super::*;

fn add_bounds(
	generics: &Generics, where_clause: Option<&WhereClause>, bounds: &[TypeParamBound]
) -> WhereClause {
	let mut clause = where_clause.cloned().unwrap_or(WhereClause {
		where_token: Default::default(),
		predicates: Punctuated::new()
	});

	for ty in generics.type_params() {
		let ty = &ty.ident;

		clause.predicates.push(parse_quote! { #ty: #(#bounds)+* });
	}

	clause
}

#[derive(Default)]
struct Attributes {
	from: Option<Attribute>,
	source: Option<(Attribute, usize)>
}

impl Attributes {
	fn get_attrs(&mut self, field: &mut Field, index: usize) {
		if self.from.is_none() {
			if let Some(from) = remove_attr_path(&mut field.attrs, "from") {
				self.from = Some(from);
			}
		}

		if self.source.is_none() {
			if let Some(source) = remove_attr_path(&mut field.attrs, "source") {
				self.source = Some((source, index));
			}
		}
	}
}

enum VariantFields {
	Named(Attributes, Punctuated<Ident, Token![,]>, Vec<Type>),
	Unnamed(Attributes, Punctuated<Index, Token![,]>, Vec<Type>),
	Unit
}

impl VariantFields {
	fn new(fields: &mut Fields) -> Self {
		match fields {
			Fields::Named(fields) => {
				let mut attributes = Attributes::default();
				let mut named = Punctuated::new();
				let mut types = Vec::new();

				for (index, field) in fields.named.iter_mut().enumerate() {
					attributes.get_attrs(field, index);
					named.push(field.ident.clone().unwrap());
					types.push(field.ty.clone());
				}

				Self::Named(attributes, named, types)
			}

			Fields::Unnamed(fields) => {
				let mut attributes = Attributes::default();
				let mut unnamed = Punctuated::new();
				let mut types = Vec::new();

				for (index, field) in fields.unnamed.iter_mut().enumerate() {
					attributes.get_attrs(field, index);
					types.push(field.ty.clone());

					#[allow(clippy::cast_possible_truncation)]
					unnamed.push(Index { index: index as u32, span: field.ty.span() });
				}

				Self::Unnamed(attributes, unnamed, types)
			}

			Fields::Unit => Self::Unit
		}
	}
}

struct Variant {
	display: Option<TokenStream>,
	kind: Option<Expr>,
	ident: Ident,
	fields: VariantFields
}

impl Variant {
	fn new(attrs: &mut Vec<Attribute>, ident: &Ident, fields: &mut Fields) -> Self {
		let display = remove_attr_list(attrs, "error").map(|attr| attr.tokens);
		let kind = remove_attr_name_value(attrs, "kind").map(|attr| attr.value);

		Self {
			display,
			kind,
			ident: ident.clone(),
			fields: VariantFields::new(fields)
		}
	}

	fn len(&self) -> usize {
		match &self.fields {
			VariantFields::Named(_, fields, _) => fields.len(),
			VariantFields::Unnamed(_, fields, _) => fields.len(),
			VariantFields::Unit => 0
		}
	}

	fn matcher(&self) -> TokenStream {
		let ident = &self.ident;

		let fields = match &self.fields {
			VariantFields::Named(_, fields, _) => {
				quote! { { #fields } }
			}

			VariantFields::Unnamed(_, fields, _) => {
				let names: Punctuated<_, Token![,]> = fields
					.iter()
					.map(|index| format_ident!("f{}", index.index))
					.collect();

				quote! { (#names) }
			}

			VariantFields::Unit => quote! {}
		};

		quote! { #ident #fields }
	}

	fn display(&self, base: &Option<TokenStream>, fmt: &Ident) -> Result<Option<TokenStream>> {
		let Some(display) = self.display.as_ref() else {
			return Ok(None);
		};

		let write = if parse2::<Ident>(display.clone()).is_ok_and(|ident| ident == "transparent") {
			if self.len() != 1 {
				return Err(Error::new_spanned(
					display,
					"#[error(transparent)] requires exactly one field"
				));
			}

			let field = match &self.fields {
				VariantFields::Named(_, fields, _) => fields[0].clone(),
				VariantFields::Unnamed(_, fields, _) => format_ident!("f{}", fields[0].index),
				VariantFields::Unit => unreachable!()
			};

			quote! { ::std::fmt::Display::fmt(#field, #fmt) }
		} else {
			quote! { ::std::write!(#fmt, #display) }
		};

		let matcher = self.matcher();

		Ok(Some(quote! { #base #matcher => #write }))
	}

	fn kind(&self, base: &Option<TokenStream>) -> Result<Option<TokenStream>> {
		let Some(kind) = self.kind.as_ref() else {
			return Ok(None);
		};

		let matcher = self.matcher();

		Ok(Some(quote! { #base #matcher => #kind }))
	}

	fn from(&self, base: &Option<TokenStream>) -> Result<Option<(TokenStream, Type)>> {
		let from = match &self.fields {
			VariantFields::Named(attrs, ..) => attrs.from.as_ref(),
			VariantFields::Unnamed(attrs, ..) => attrs.from.as_ref(),
			VariantFields::Unit => None
		};

		let Some(attr) = from else {
			return Ok(None);
		};

		if self.len() != 1 {
			return Err(Error::new_spanned(
				attr,
				"#[from] requires exactly one field"
			));
		}

		let (field, ty) = match &self.fields {
			VariantFields::Named(_, fields, types) => (Member::Named(fields[0].clone()), &types[0]),
			VariantFields::Unnamed(_, fields, types) => {
				(Member::Unnamed(fields[0].clone()), &types[0])
			}

			VariantFields::Unit => unreachable!()
		};

		let ident = &self.ident;

		Ok(Some((
			quote! { #base #ident { #field: value } },
			ty.clone()
		)))
	}

	fn source(&self, base: &Option<TokenStream>) -> Result<Option<TokenStream>> {
		let from = match &self.fields {
			VariantFields::Named(attrs, ..) => attrs.source.as_ref(),
			VariantFields::Unnamed(attrs, ..) => attrs.source.as_ref(),
			VariantFields::Unit => None
		};

		let Some((_, index)) = from else {
			return Ok(None);
		};

		let field = match &self.fields {
			VariantFields::Named(_, fields, _) => fields[*index].clone(),
			VariantFields::Unnamed(_, fields, _) => format_ident!("f{}", fields[*index].index),
			VariantFields::Unit => unreachable!()
		};

		let matcher = self.matcher();

		Ok(Some(quote! { #base #matcher => Some(#field) }))
	}
}

enum Repr {
	Struct(Variant),
	Enum(Vec<Variant>)
}

struct Input {
	input: DeriveInput,
	repr: Repr
}

impl Input {
	fn parse_struct(
		attrs: &mut Vec<Attribute>, ident: &Ident, item: &mut DataStruct
	) -> Result<Variant> {
		Ok(Variant::new(attrs, ident, &mut item.fields))
	}

	fn parse_enum(item: &mut DataEnum) -> Result<Vec<Variant>> {
		let mut variants = Vec::new();

		for variant in &mut item.variants {
			variants.push(Variant::new(
				&mut variant.attrs,
				&variant.ident,
				&mut variant.fields
			));
		}

		Ok(variants)
	}

	fn parse(item: TokenStream) -> Result<Self> {
		let mut input: DeriveInput = parse2(item)?;

		input.attrs.push(parse_quote! {
			#[derive(::std::fmt::Debug)]
		});

		input.attrs.push(parse_quote! {
			#[allow(missing_copy_implementations)]
		});

		let repr = match &mut input.data {
			Data::Struct(data) => {
				Repr::Struct(Self::parse_struct(&mut input.attrs, &input.ident, data)?)
			}

			Data::Enum(data) => Repr::Enum(Self::parse_enum(data)?),
			Data::Union(_) => return Err(Error::new_spanned(input, "Unions are not supported"))
		};

		Ok(Self { input, repr })
	}

	fn expand(&self) -> Result<TokenStream> {
		let name = &self.input.ident;
		let (impl_generics, type_generics, where_clause) = self.input.generics.split_for_impl();

		let fmt = Ident::new("fmt", Span::mixed_site());
		let base = Some(quote! { Self:: });

		let mut displays = Punctuated::<_, Token![,]>::new();
		let mut kinds = Punctuated::<_, Token![,]>::new();
		let mut sources = Punctuated::<_, Token![,]>::new();
		let mut froms = Vec::new();

		let eq = match &self.repr {
			Repr::Struct(variant) => {
				if let Some(display) = variant.display(&None, &fmt)? {
					displays.push(display);
				}

				if let Some(kind) = variant.kind(&None)? {
					kinds.push(kind);
				}

				if let Some(from) = variant.from(&None)? {
					froms.push(from);
				}

				if let Some(source) = variant.source(&None)? {
					sources.push(source);
				}

				quote! { true }
			}

			Repr::Enum(variants) => {
				let has_display = variants.iter().any(|variant| variant.display.is_some());

				for variant in variants {
					if has_display {
						let Some(display) = variant.display(&base, &fmt)? else {
							return Err(Error::new_spanned(
								&variant.ident,
								"Missing #[error(..)] display attribute"
							));
						};

						displays.push(display);
					}

					if let Some(kind) = variant.kind(&base)? {
						kinds.push(kind);
					}

					if let Some(from) = variant.from(&base)? {
						froms.push(from);
					}

					if let Some(source) = variant.source(&base)? {
						sources.push(source);
					}
				}

				quote! {
					::std::mem::discriminant(self) == ::std::mem::discriminant(other)
				}
			}
		};

		let display = (!displays.is_empty()).then(|| {
			let where_clause = add_bounds(
				&self.input.generics,
				where_clause,
				&[parse_quote! { ::std::fmt::Display }]
			);

			quote! {
				impl #impl_generics ::std::fmt::Display for #name #type_generics #where_clause {
					fn fmt(&self, #fmt: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
						#[allow(unused_variables)]
						match self {
							#displays
						}
					}
				}
			}
		});

		let mut from_impls = Vec::new();

		for (body, ty) in froms {
			from_impls.push(quote_spanned! { ty.span() =>
				impl #impl_generics ::std::convert::From<#ty> for #name #type_generics #where_clause {
					fn from(value: #ty) -> Self {
						#body
					}
				}
			});
		}

		kinds.push(quote! { _ => ::xx_core::error::ErrorKind::Other });
		sources.push(quote! { _ => None });

		Ok(quote! {
			#display
			#(#from_impls)*

			impl #impl_generics ::std::cmp::PartialEq for #name #type_generics #where_clause {
				fn eq(&self, other: &Self) -> bool {
					#eq
				}
			}

			impl #impl_generics ::std::error::Error for #name #type_generics #where_clause
			where
				Self: ::std::fmt::Debug + ::std::fmt::Display
			{
				fn source(&self) -> ::std::option::Option<&(dyn ::std::error::Error + 'static)> {
					#[allow(unused_variables)]
					match self {
						#sources
					}
				}
			}

			impl #impl_generics ::xx_core::error::internal::ErrorImpl for #name #type_generics #where_clause
			where
				Self: ::std::error::Error + ::std::marker::Send
					+::std::marker::Sync + 'static
			{
				fn kind(&self) -> ErrorKind {
					#[allow(unused_variables)]
					match self {
						#kinds
					}
				}
			}
		})
	}
}

fn expand(item: TokenStream) -> Result<TokenStream> {
	let input = Input::parse(item)?;
	let orig = &input.input;
	let expansion = input.expand()?;

	Ok(quote! {
		#orig
		#expansion
	})
}

pub fn error(_: TokenStream, item: TokenStream) -> TokenStream {
	try_expand(|| expand(item))
}
