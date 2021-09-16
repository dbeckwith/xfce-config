#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

use heck::CamelCase;
use proc_macro_error::proc_macro_error;
use quote::{format_ident, quote};
use syn::{
    braced,
    bracketed,
    parenthesized,
    parse::{Parse, ParseStream, Parser},
    parse_quote,
    punctuated::Punctuated,
    token::{Brace, Bracket, Paren},
    Arm,
    Error,
    Ident,
    Item,
    ItemFn,
    LitStr,
    Result,
    Token,
};

#[proc_macro]
#[proc_macro_error]
pub fn config_types(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let config_fields = match Punctuated::parse_terminated.parse(input) {
        Ok(config_fields) => config_fields,
        Err(error) => return error.to_compile_error().into(),
    };
    let config_type = Type::Struct(TypeStruct {
        brace_token: Brace::default(),
        fields: config_fields,
    });
    // eprintln!("{:#?}", config_type);
    let mut type_decls = Vec::new();
    match config_type
        .emit_type_decls(Path(vec![format_ident!("config")]), &mut type_decls)
    {
        Ok(()) => {},
        Err(error) => return error.to_compile_error().into(),
    }
    (quote! {
        #(#type_decls)*
    })
    .into()
}

trait EmitTypeDecls {
    fn emit_type_decls(
        self,
        path: Path,
        type_decls: &mut Vec<Item>,
    ) -> Result<()>;
}

#[derive(Clone)]
struct Path(Vec<Ident>);

impl Path {
    fn push(mut self, part: Ident) -> Self {
        self.0.push(part);
        self
    }

    fn singular(mut self) -> Self {
        let word = self.0.pop().unwrap();
        let span = word.span();
        let word = word.to_string();
        let word = word.trim_end_matches('s');
        let word = Ident::new(word, span);
        self.0.push(word);
        self
    }

    fn join(self) -> Ident {
        let ident = self
            .0
            .into_iter()
            .reduce(|a, b| format_ident!("{}_{}", a, b))
            .unwrap();
        let span = ident.span();
        Ident::new(ident.to_string().to_camel_case().as_str(), span)
    }
}

mod kw {
    syn::custom_keyword!(bool);
    syn::custom_keyword!(int);
    syn::custom_keyword!(uint);
    syn::custom_keyword!(str);
    syn::custom_keyword!(color);
}

#[derive(Debug)]
enum Type {
    Bool(TypeBool),
    Int(TypeInt),
    Uint(TypeUint),
    Str(TypeStr),
    Color(TypeColor),
    LitStr(TypeLitStr),
    Array(TypeArray),
    Struct(TypeStruct),
    Enum(TypeEnum),
}

#[derive(Debug)]
struct TypeBool {
    bool_token: kw::bool,
}

#[derive(Debug)]
struct TypeInt {
    int_token: kw::int,
}

#[derive(Debug)]
struct TypeUint {
    uint_token: kw::uint,
}

#[derive(Debug)]
struct TypeStr {
    str_token: kw::str,
}

#[derive(Debug)]
struct TypeColor {
    color_token: kw::color,
}

#[derive(Debug)]
struct TypeLitStr {
    lit_str: LitStr,
}

#[derive(Debug)]
struct TypeArray {
    bracket_token: Bracket,
    element_ty: Box<Type>,
}

#[derive(Debug)]
struct TypeStruct {
    brace_token: Brace,
    fields: Punctuated<Field, Token![;]>,
}

#[derive(Debug)]
struct Field {
    name: Ident,
    colon_token: Token![:],
    ty: Type,
}

#[derive(Debug)]
struct TypeEnum {
    paren_token: Paren,
    or_token: Option<Token![|]>,
    variants: Punctuated<Type, Token![|]>,
}

impl Parse for Type {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(kw::bool) {
            input.parse().map(Self::Bool)
        } else if lookahead.peek(kw::int) {
            input.parse().map(Self::Int)
        } else if lookahead.peek(kw::uint) {
            input.parse().map(Self::Uint)
        } else if lookahead.peek(kw::str) {
            input.parse().map(Self::Str)
        } else if lookahead.peek(kw::color) {
            input.parse().map(Self::Color)
        } else if lookahead.peek(LitStr) {
            input.parse().map(Self::LitStr)
        } else if lookahead.peek(Bracket) {
            input.parse().map(Self::Array)
        } else if lookahead.peek(Brace) {
            input.parse().map(Self::Struct)
        } else if lookahead.peek(Paren) {
            input.parse().map(Self::Enum)
        } else {
            Err(lookahead.error())
        }
    }
}

impl Parse for TypeBool {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let bool_token = input.parse()?;
        Ok(Self { bool_token })
    }
}

impl Parse for TypeInt {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let int_token = input.parse()?;
        Ok(Self { int_token })
    }
}

impl Parse for TypeUint {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let uint_token = input.parse()?;
        Ok(Self { uint_token })
    }
}

impl Parse for TypeStr {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let str_token = input.parse()?;
        Ok(Self { str_token })
    }
}

impl Parse for TypeColor {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let color_token = input.parse()?;
        Ok(Self { color_token })
    }
}

impl Parse for TypeLitStr {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let lit_str = input.parse()?;
        Ok(Self { lit_str })
    }
}

impl Parse for TypeStruct {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        let brace_token = braced!(content in input);
        let fields = content.parse_terminated(Parse::parse)?;
        Ok(Self {
            brace_token,
            fields,
        })
    }
}

impl Parse for Field {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        let colon_token = input.parse()?;
        let ty = input.parse()?;
        Ok(Self {
            name,
            colon_token,
            ty,
        })
    }
}

impl Parse for TypeArray {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        let bracket_token = bracketed!(content in input);
        let element_ty = content.parse()?;
        Ok(Self {
            bracket_token,
            element_ty,
        })
    }
}

impl Parse for TypeEnum {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        let paren_token = parenthesized!(content in input);
        let or_token = content.parse()?;
        let variants = Punctuated::parse_separated_nonempty(&content)?;
        Ok(Self {
            paren_token,
            or_token,
            variants,
        })
    }
}

impl EmitTypeDecls for Type {
    fn emit_type_decls(
        self,
        path: Path,
        type_decls: &mut Vec<Item>,
    ) -> Result<()> {
        match self {
            Type::Bool(type_bool) => {
                type_bool.emit_type_decls(path, type_decls)
            },
            Type::Int(type_int) => type_int.emit_type_decls(path, type_decls),
            Type::Uint(type_uint) => {
                type_uint.emit_type_decls(path, type_decls)
            },
            Type::Str(type_str) => type_str.emit_type_decls(path, type_decls),
            Type::Color(type_color) => {
                type_color.emit_type_decls(path, type_decls)
            },
            Type::LitStr(type_lit_str) => {
                type_lit_str.emit_type_decls(path, type_decls)
            },
            Type::Array(type_array) => {
                type_array.emit_type_decls(path, type_decls)
            },
            Type::Struct(type_struct) => {
                type_struct.emit_type_decls(path, type_decls)
            },
            Type::Enum(type_enum) => {
                type_enum.emit_type_decls(path, type_decls)
            },
        }
    }
}

impl EmitTypeDecls for TypeBool {
    fn emit_type_decls(
        self,
        _path: Path,
        _type_decls: &mut Vec<Item>,
    ) -> Result<()> {
        Ok(())
    }
}

impl EmitTypeDecls for TypeInt {
    fn emit_type_decls(
        self,
        _path: Path,
        _type_decls: &mut Vec<Item>,
    ) -> Result<()> {
        Ok(())
    }
}

impl EmitTypeDecls for TypeUint {
    fn emit_type_decls(
        self,
        _path: Path,
        _type_decls: &mut Vec<Item>,
    ) -> Result<()> {
        Ok(())
    }
}

impl EmitTypeDecls for TypeStr {
    fn emit_type_decls(
        self,
        _path: Path,
        _type_decls: &mut Vec<Item>,
    ) -> Result<()> {
        Ok(())
    }
}

impl EmitTypeDecls for TypeColor {
    fn emit_type_decls(
        self,
        _path: Path,
        _type_decls: &mut Vec<Item>,
    ) -> Result<()> {
        Ok(())
    }
}

impl EmitTypeDecls for TypeLitStr {
    fn emit_type_decls(
        self,
        _path: Path,
        _type_decls: &mut Vec<Item>,
    ) -> Result<()> {
        Ok(())
    }
}

impl EmitTypeDecls for TypeArray {
    fn emit_type_decls(
        self,
        path: Path,
        type_decls: &mut Vec<Item>,
    ) -> Result<()> {
        let Self {
            bracket_token: _,
            element_ty,
        } = self;
        let path = path.singular();
        element_ty.emit_type_decls(path, type_decls)?;
        Ok(())
    }
}

impl EmitTypeDecls for TypeStruct {
    fn emit_type_decls(
        self,
        path: Path,
        type_decls: &mut Vec<Item>,
    ) -> Result<()> {
        let Self {
            brace_token: _,
            fields,
        } = self;
        let mut decl_fields = Vec::<syn::Field>::new();
        for Field {
            name,
            colon_token: _,
            ty,
        } in fields
        {
            let path = path.clone().push(name.clone());
            let ty_name: syn::Type = match ty {
                Type::Bool(_) => parse_quote!(bool),
                Type::Int(_) => parse_quote!(i32),
                Type::Uint(_) => parse_quote!(u32),
                Type::Str(_) => parse_quote!(::std::string::String),
                Type::Color(_) => parse_quote!(Color),
                Type::LitStr(TypeLitStr { lit_str }) => {
                    return Err(Error::new_spanned(
                        lit_str,
                        "fields cannot have literal strings as types",
                    ))
                },
                Type::Array(_) => {
                    let ty_name = path.clone().singular().join();
                    parse_quote!(::std::vec::Vec<#ty_name>)
                },
                Type::Struct(_) => {
                    let ty_name = path.clone().join();
                    parse_quote!(#ty_name)
                },
                Type::Enum(_) => {
                    let ty_name = path.clone().join();
                    parse_quote!(#ty_name)
                },
            };
            ty.emit_type_decls(path, type_decls)?;
            let field = syn::Field::parse_named
                .parse2(quote! {
                    pub #name: Option<#ty_name>
                })
                .unwrap();
            decl_fields.push(field);
        }
        let name = path.join();
        let type_decl = parse_quote! {
            pub struct #name {
                #(#decl_fields,)*
            }
        };
        type_decls.push(type_decl);
        Ok(())
    }
}

impl EmitTypeDecls for TypeEnum {
    fn emit_type_decls(
        self,
        path: Path,
        type_decls: &mut Vec<Item>,
    ) -> Result<()> {
        let Self {
            paren_token,
            or_token: _,
            variants,
        } = self;
        let mut decl_variants = Vec::<syn::Variant>::new();
        let mut discrim_fns = Vec::<ItemFn>::new();
        if variants.iter().all(|ty| matches!(ty, Type::LitStr(_))) {
            discrim_fns.extend(emit_enum_lit_str(variants, &mut decl_variants)?);
        } else if variants.iter().all(|ty| matches!(ty, Type::Struct(_))) {
            discrim_fns.extend(emit_enum_struct(
                variants,
                &mut decl_variants,
                path.clone(),
                type_decls,
            )?);
        } else if variants.iter().all(|ty| {
            !matches!(ty, Type::LitStr(_) | Type::Array(_) | Type::Enum(_))
        }) && is_unique_types(variants.iter())
        {
            emit_enum_unique(
                variants,
                &mut decl_variants,
                path.clone(),
                type_decls,
            )?;
        } else {
            return Err(Error::new(paren_token.span, "bad enum type"));
        }
        let name = path.join();
        let type_decl = parse_quote! {
            pub enum #name {
                #(#decl_variants,)*
            }
        };
        type_decls.push(type_decl);
        if !discrim_fns.is_empty() {
            let discrim_impl = parse_quote! {
                impl #name {
                    #(#discrim_fns)*
                }
            };
            type_decls.push(discrim_impl);
        }
        Ok(())
    }
}

fn emit_enum_lit_str(
    variants: impl IntoIterator<Item = Type>,
    decl_variants: &mut Vec<syn::Variant>,
) -> Result<Vec<ItemFn>> {
    let mut discrim_values = Vec::new();
    for ty in variants {
        let TypeLitStr { lit_str } = match ty {
            Type::LitStr(type_lit_str) => type_lit_str,
            _ => unreachable!(),
        };
        let variant_name = lit_str_to_ident(&lit_str);
        discrim_values.push((variant_name.clone(), lit_str));
        let variant = parse_quote! {
            #variant_name
        };
        decl_variants.push(variant);
    }
    let name_match_arms =
        discrim_values
            .iter()
            .map(|(variant_name, variant_lit_str)| -> Arm {
                parse_quote! {
                    Self::#variant_name => #variant_lit_str
                }
            });
    let discrim_match_arms = discrim_values.iter().enumerate().map(
        |(variant_idx, (variant_name, _variant_lit_str))| -> Arm {
            let variant_idx = variant_idx as u32;
            parse_quote! {
                Self::#variant_name => #variant_idx
            }
        },
    );
    Ok(vec![
        parse_quote! {
            pub fn name(&self) -> &'static str {
                match self {
                    #(#name_match_arms,)*
                }
            }
        },
        parse_quote! {
            pub fn discrim(&self) -> u32 {
                match self {
                    #(#discrim_match_arms,)*
                }
            }
        },
    ])
}

fn emit_enum_struct(
    variants: impl IntoIterator<Item = Type>,
    decl_variants: &mut Vec<syn::Variant>,
    path: Path,
    type_decls: &mut Vec<Item>,
) -> Result<Vec<ItemFn>> {
    let mut discrim_field_name = None::<Ident>;
    let mut discrim_values = Vec::new();
    for ty in variants {
        let TypeStruct {
            brace_token,
            fields,
        } = match ty {
            Type::Struct(type_struct) => type_struct,
            _ => unreachable!(),
        };
        let mut fields = fields.into_iter();
        let discrim_field = fields.next().ok_or_else(|| {
            Error::new(brace_token.span, "at least one field is required")
        })?;
        if let Some(discrim_field_name) = discrim_field_name.as_ref() {
            if &discrim_field.name != discrim_field_name {
                return Err(Error::new_spanned(
                    discrim_field.name,
                    "first field must have the same name for all variants",
                ));
            }
        } else {
            discrim_field_name = Some(discrim_field.name.clone());
        }
        let variant_name_lit_str = {
            let Field {
                name,
                colon_token: _,
                ty,
            } = &discrim_field;
            match &ty {
                Type::LitStr(TypeLitStr { lit_str }) => lit_str,
                _ => {
                    return Err(Error::new_spanned(
                        name,
                        "first field must be a literal string",
                    ))
                },
            }
        };
        let variant_name = lit_str_to_ident(variant_name_lit_str);
        discrim_values
            .push((variant_name.clone(), variant_name_lit_str.clone()));
        let path = path.clone().push(variant_name.clone());
        let fields = fields.collect::<Punctuated<_, _>>();
        if fields.is_empty() {
            let variant = parse_quote! {
                #variant_name
            };
            decl_variants.push(variant);
        } else {
            TypeStruct {
                brace_token,
                fields,
            }
            .emit_type_decls(path.clone(), type_decls)?;
            let variant_ty: syn::Type = {
                let ty_name = path.join();
                parse_quote!(#ty_name)
            };
            let variant = parse_quote! {
                #variant_name(#variant_ty)
            };
            decl_variants.push(variant);
        }
    }
    let discrim_field_name = discrim_field_name.unwrap();
    let match_arms = discrim_values.into_iter().map(
        |(variant_name, variant_name_lit_str)| -> Arm {
            parse_quote! {
                Self::#variant_name { .. } => #variant_name_lit_str
            }
        },
    );
    Ok(vec![parse_quote! {
        pub fn #discrim_field_name(&self) -> &'static str {
            match self {
                #(#match_arms,)*
            }
        }
    }])
}

fn emit_enum_unique(
    variants: impl IntoIterator<Item = Type>,
    decl_variants: &mut Vec<syn::Variant>,
    path: Path,
    type_decls: &mut Vec<Item>,
) -> Result<()> {
    for ty in variants {
        let variant_name = format_ident!(
            "{}",
            match ty {
                Type::Bool(_) => "Bool",
                Type::Int(_) => "Int",
                Type::Uint(_) => "Uint",
                Type::Str(_) => "Str",
                Type::Color(_) => "Color",
                Type::LitStr(_) => unreachable!(),
                Type::Array(_) => unreachable!(),
                Type::Struct(_) => "Struct",
                Type::Enum(_) => unreachable!(),
            }
        );
        let path = path.clone().push(variant_name.clone());
        let variant_ty: syn::Type = match ty {
            Type::Bool(_) => parse_quote!(bool),
            Type::Int(_) => parse_quote!(i32),
            Type::Uint(_) => parse_quote!(u32),
            Type::Str(_) => parse_quote!(::std::string::String),
            Type::Color(_) => parse_quote!(Color),
            Type::LitStr(_) => unreachable!(),
            Type::Array(_) => unreachable!(),
            Type::Struct(_) => {
                let ty_name = path.clone().join();
                parse_quote!(#ty_name)
            },
            Type::Enum(_) => unreachable!(),
        };
        ty.emit_type_decls(path, type_decls)?;
        let variant = parse_quote! {
            #variant_name(#variant_ty)
        };
        decl_variants.push(variant);
    }
    Ok(())
}

fn is_unique_types<'a>(types: impl Iterator<Item = &'a Type>) -> bool {
    let mut seen_bool = false;
    let mut seen_int = false;
    let mut seen_str = false;
    let mut seen_color = false;
    let mut seen_struct = false;
    for ty in types {
        match ty {
            Type::Bool(_) => {
                if seen_bool {
                    return false;
                }
                seen_bool = true;
            },
            Type::Int(_) => {
                if seen_int {
                    return false;
                }
                seen_int = true;
            },
            Type::Uint(_) => {
                if seen_int {
                    return false;
                }
                seen_int = true;
            },
            Type::Str(_) => {
                if seen_str {
                    return false;
                }
                seen_str = true;
            },
            Type::Color(_) => {
                if seen_color {
                    return false;
                }
                seen_color = true;
            },
            Type::LitStr(_) => unreachable!(),
            Type::Array(_) => unreachable!(),
            Type::Struct(_) => {
                if seen_struct {
                    return false;
                }
                seen_struct = true;
            },
            Type::Enum(_) => unreachable!(),
        }
    }
    true
}

fn lit_str_to_ident(lit_str: &LitStr) -> Ident {
    Ident::new(
        lit_str.value().as_str().to_camel_case().as_str(),
        lit_str.span(),
    )
}
