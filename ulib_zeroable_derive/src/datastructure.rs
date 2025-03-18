use syn::{
    self, Attribute, Data, DeriveInput, Field as SynField, Fields as SynFields, Generics, Ident,
    Type, Visibility,
};

use quote::ToTokens;

use proc_macro2::{Span, TokenStream};

use std::fmt::{self, Display};

//////////////////////////////////////////////////////////////////////////////

/// A type definition(enum,struct,union).
#[derive(Clone)]
pub struct DataStructure<'a> {
    pub vis: &'a Visibility,
    pub name: &'a Ident,
    pub generics: &'a Generics,

    pub attrs: &'a [Attribute],

    /// Whether this is a struct/union/enum.
    pub data_variant: DataVariant,

    /// The variants in the type definition.
    ///
    /// If it is a struct or a union this only has 1 element.
    pub variants: Vec<Struct<'a>>,
}

impl<'a> DataStructure<'a> {
    pub fn new(ast: &'a DeriveInput) -> Self {
        let name = &ast.ident;

        let data_variant: DataVariant;

        let mut variants = Vec::new();

        match &ast.data {
            Data::Enum(enum_) => {
                for (variant, var) in enum_.variants.iter().enumerate() {
                    variants.push(Struct::new(
                        StructParams {
                            discriminant: var.discriminant.as_ref().map(|(_, v)| v),
                            variant: variant,
                            attrs: &var.attrs,
                            name: &var.ident,
                            override_vis: Some(&ast.vis),
                        },
                        &var.fields,
                    ));
                }
                data_variant = DataVariant::Enum;
            }
            Data::Struct(struct_) => {
                variants.push(Struct::new(
                    StructParams {
                        discriminant: None,
                        variant: 0,
                        attrs: &[],
                        name: name,
                        override_vis: None,
                    },
                    &struct_.fields,
                ));
                data_variant = DataVariant::Struct;
            }

            Data::Union(union_) => {
                let vari = Struct::with_fields(
                    StructParams {
                        discriminant: None,
                        variant: 0,
                        attrs: &[],
                        name: name,
                        override_vis: None,
                    },
                    Some(&union_.fields.named),
                );
                variants.push(vari);
                data_variant = DataVariant::Union;
            }
        }

        Self {
            vis: &ast.vis,
            name,
            attrs: &ast.attrs,
            generics: &ast.generics,
            data_variant,
            variants,
        }
    }

    pub fn is_public(&self) -> bool {
        match self.vis {
            Visibility::Public { .. } => true,
            _ => false,
        }
    }
}

//////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub enum DataVariant {
    Struct,
    Enum,
    Union,
}

#[derive(Copy, Clone, Debug, PartialEq, Hash)]
pub struct FieldIndex {
    pub variant: usize,
    pub pos: usize,
}

//////////////////////////////////////////////////////////////////////////////

#[derive(Copy, Clone)]
struct StructParams<'a> {
    discriminant: Option<&'a syn::Expr>,
    variant: usize,
    attrs: &'a [Attribute],
    name: &'a Ident,
    override_vis: Option<&'a Visibility>,
}

/// A struct/union or a variant of an enum.
#[derive(Clone)]
pub struct Struct<'a> {
    /// The attributes of this `Struct`.
    ///
    /// If this is a struct/union:these is the same as DataStructure.attrs.
    ///
    /// If this is an enum:these are the attributes on the variant.
    pub attrs: &'a [Attribute],
    /// The name of this `Struct`.
    ///
    /// If this is a struct/union:these is the same as DataStructure.name.
    ///
    /// If this is an enum:this is the name of the variant.
    pub name: &'a Ident,
    pub fields: Vec<MyField<'a>>,
    /// The value of this discriminant.
    ///
    /// If this is a Some(_):This is an enum with an explicit discriminant value.
    ///
    /// If this is an None:
    ///     This is either a struct/union or an enum variant without an explicit discriminant.
    pub discriminant: Option<&'a syn::Expr>,
    _priv: (),
}

impl<'a> Struct<'a> {
    fn new(p: StructParams<'a>, fields: &'a SynFields) -> Self {
        let fields = match fields {
            SynFields::Named(f) => Some(&f.named),
            SynFields::Unnamed(f) => Some(&f.unnamed),
            SynFields::Unit => None,
        };

        Self::with_fields(p, fields)
    }

    fn with_fields<I>(p: StructParams<'a>, fields: Option<I>) -> Self
    where
        I: IntoIterator<Item = &'a SynField>,
    {
        let fields = match fields {
            Some(x) => MyField::from_iter(p, x),
            None => Vec::new(),
        };

        Self {
            discriminant: p.discriminant,
            attrs: p.attrs,
            name: p.name,
            fields,
            _priv: (),
        }
    }
}

//////////////////////////////////////////////////////////////////////////////

/// Represent a struct field
///
#[derive(Clone)]
pub struct MyField<'a> {
    pub index: FieldIndex,
    pub attrs: &'a [Attribute],
    pub vis: &'a Visibility,
    /// identifier for the field,which is either an index(in a tuple struct) or a name.
    pub ident: FieldIdent<'a>,
    pub ty: &'a Type,
}

impl<'a> MyField<'a> {
    fn new(
        index: FieldIndex,
        field: &'a SynField,
        span: Span,
        override_vis: Option<&'a Visibility>,
    ) -> Self {
        let ident = match field.ident.as_ref() {
            Some(ident) => FieldIdent::Named(ident),
            None => FieldIdent::new_index(index.pos, span),
        };

        Self {
            index,
            attrs: &field.attrs,
            vis: override_vis.unwrap_or(&field.vis),
            ident,
            ty: &field.ty,
        }
    }

    fn from_iter<I>(p: StructParams<'a>, fields: I) -> Vec<Self>
    where
        I: IntoIterator<Item = &'a SynField>,
    {
        fields
            .into_iter()
            .enumerate()
            .map(|(pos, f)| {
                let fi = FieldIndex {
                    variant: p.variant,
                    pos,
                };
                MyField::new(fi, f, p.name.span(), p.override_vis)
            })
            .collect()
    }

    /// Gets the identifier of this field as an `&Ident`.
    #[allow(dead_code)]
    pub fn ident(&self) -> &Ident {
        match &self.ident {
            FieldIdent::Index(_, ident) => ident,
            FieldIdent::Named(ident) => ident,
        }
    }

    pub fn ty_span(&self) -> Span {
        use syn::spanned::Spanned;
        self.ty.span()
    }

    pub fn is_public(&self) -> bool {
        match self.vis {
            Visibility::Public { .. } => true,
            _ => false,
        }
    }
}

//////////////////////////////////////////////////////////////////////////////

#[derive(Clone)]
pub enum FieldIdent<'a> {
    Index(usize, Ident),
    Named(&'a Ident),
}

impl<'a> Display for FieldIdent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldIdent::Index(x, ..) => Display::fmt(x, f),
            FieldIdent::Named(x) => Display::fmt(x, f),
        }
    }
}

impl<'a> ToTokens for FieldIdent<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match *self {
            FieldIdent::Index(ind, ..) => syn::Index::from(ind).to_tokens(tokens),
            FieldIdent::Named(name) => name.to_tokens(tokens),
        }
    }
}

impl<'a> FieldIdent<'a> {
    fn new_index(index: usize, span: Span) -> Self {
        FieldIdent::Index(index, Ident::new(&format!("field_{}", index), span))
    }
}
