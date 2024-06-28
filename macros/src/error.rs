use super::*;

#[derive(Default)]
struct Attributes {
	from: Option<Attribute>,
	source: Option<(Attribute, usize)>
}

impl Attributes {
	fn get_attrs(&mut self, field: &mut Field, index: usize) {
		if self.from.is_none() {
			self.from = remove_attr_path(&mut field.attrs, "from");
		}

		if self.source.is_none() {
			let attr = remove_attr_path(&mut field.attrs, "source");

			self.source = attr.map(|source| (source, index));
		}
	}
}

struct VariantFields {
	attributes: Attributes,
	members: Vec<(Member, Type)>,
	named: bool
}

impl VariantFields {
	fn new(fields: &mut Fields) -> Self {
		let mut attributes = Attributes::default();
		let mut members = Vec::new();

		match fields {
			Fields::Named(fields) => {
				for (index, field) in fields.named.iter_mut().enumerate() {
					attributes.get_attrs(field, index);

					let ident = field.ident.clone().unwrap();

					members.push((Member::Named(ident), field.ty.clone()));
				}
			}

			Fields::Unnamed(fields) => {
				for (index, field) in fields.unnamed.iter_mut().enumerate() {
					attributes.get_attrs(field, index);

					#[allow(clippy::cast_possible_truncation)]
					let index = Index { index: index as u32, span: field.ty.span() };

					members.push((Member::Unnamed(index), field.ty.clone()));
				}
			}

			Fields::Unit => ()
		}

		Self {
			attributes,
			members,
			named: !matches!(fields, Fields::Unnamed(_))
		}
	}
}

fn member_as_ident(member: &Member) -> Ident {
	match member {
		Member::Named(ident) => ident.clone(),
		Member::Unnamed(index) => format_ident!("f{}", index.index)
	}
}

struct Variant {
	display: Option<(TokenStream, bool)>,
	kind: Option<Expr>,
	ident: Ident,
	fields: VariantFields
}

impl Variant {
	fn new(attrs: &mut Vec<Attribute>, ident: &Ident, fields: &mut Fields) -> Result<Self> {
		let display = remove_attr_list(attrs, "error").map(|attr| attr.tokens);
		let kind = remove_attr_name_value(attrs, "kind").map(|attr| attr.value);

		let mut this = Self {
			display: display.map(|display| (display, false)),
			kind,
			ident: ident.clone(),
			fields: VariantFields::new(fields)
		};

		if let Some((display, is_transparent)) = &mut this.display {
			*is_transparent =
				parse2::<Ident>(display.clone()).is_ok_and(|ident| ident == "transparent");
		}

		if this.fields.members.len() == 1 {
			return Ok(this);
		}

		if let Some((display, is_transparent)) = &this.display {
			if *is_transparent {
				let msg = "#[error(transparent)] requires exactly one field";

				return Err(Error::new_spanned(display, msg));
			}
		}

		if let Some(from) = &this.fields.attributes.from {
			let msg = "#[from] requires exactly one field";

			return Err(Error::new_spanned(from, msg));
		}

		Ok(this)
	}

	fn matcher(&self) -> TokenStream {
		let ident = &self.ident;
		let mut fields = Punctuated::<Ident, Token![,]>::new();

		for (member, _) in &self.fields.members {
			fields.push(member_as_ident(member));
		}

		if self.fields.named {
			quote! { #ident { #fields } }
		} else {
			quote! { #ident ( #fields ) }
		}
	}

	fn display(&self, base: &Option<TokenStream>, fmt: &Ident) -> Option<TokenStream> {
		let (display, is_transparent) = self.display.as_ref()?;
		let write = if *is_transparent {
			let field = member_as_ident(&self.fields.members[0].0);

			quote! { ::std::fmt::Display::fmt(#field, #fmt) }
		} else {
			quote! { ::std::write!(#fmt, #display) }
		};

		let matcher = self.matcher();

		Some(quote! { #base #matcher => #write })
	}

	fn kind(&self, base: &Option<TokenStream>) -> Option<TokenStream> {
		let kind = self.kind.as_ref()?;
		let matcher = self.matcher();

		Some(quote! { #base #matcher => #kind })
	}

	fn from(&self, base: &Option<TokenStream>) -> Option<(TokenStream, &Type)> {
		let _ = self.fields.attributes.from.as_ref()?;

		let ident = &self.ident;
		let (member, ty) = &self.fields.members[0];

		Some((quote! { #base #ident { #member: value } }, ty))
	}

	fn source(&self, base: &Option<TokenStream>) -> Option<TokenStream> {
		let (_, index) = self.fields.attributes.source.as_ref()?;

		let matcher = self.matcher();
		let member = member_as_ident(&self.fields.members[*index].0);

		Some(quote! { #base #matcher => Some(#member) })
	}
}

enum Repr {
	Struct(Variant),
	Enum(Vec<Variant>)
}

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

struct Input {
	input: DeriveInput,
	repr: Repr
}

impl Input {
	fn parse_enum(item: &mut DataEnum) -> Result<Vec<Variant>> {
		let mut variants = Vec::new();

		for variant in &mut item.variants {
			variants.push(Variant::new(
				&mut variant.attrs,
				&variant.ident,
				&mut variant.fields
			)?);
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
			Data::Struct(data) => Repr::Struct(Variant::new(
				&mut input.attrs,
				&input.ident,
				&mut data.fields
			)?),

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
				if let Some(display) = variant.display(&None, &fmt) {
					displays.push(display);
				}

				if let Some(kind) = variant.kind(&None) {
					kinds.push(kind);
				}

				if let Some(from) = variant.from(&None) {
					froms.push(from);
				}

				if let Some(source) = variant.source(&None) {
					sources.push(source);
				}

				quote! { true }
			}

			Repr::Enum(variants) => {
				let has_display = variants.iter().any(|variant| variant.display.is_some());

				for variant in variants {
					if has_display {
						let Some(display) = variant.display(&base, &fmt) else {
							return Err(Error::new_spanned(
								&variant.ident,
								"Missing #[error(..)] display attribute"
							));
						};

						displays.push(display);
					}

					if let Some(kind) = variant.kind(&base) {
						kinds.push(kind);
					}

					if let Some(from) = variant.from(&base) {
						froms.push(from);
					}

					if let Some(source) = variant.source(&base) {
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
					+ ::std::marker::Sync + 'static
			{
				fn kind(&self) -> ::xx_core::error::ErrorKind {
					#[allow(unused_variables)]
					match self {
						#kinds
					}
				}
			}
		})
	}
}

fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
	ensure_empty(attr)?;

	let input = Input::parse(item)?;
	let orig = &input.input;
	let expansion = input.expand()?;

	Ok(quote! {
		#orig
		#expansion
	})
}

pub fn error(attr: TokenStream, item: TokenStream) -> TokenStream {
	try_expand(|| expand(attr, item))
}
