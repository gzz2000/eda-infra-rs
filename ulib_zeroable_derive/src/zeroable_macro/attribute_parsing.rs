use crate::{
    attribute_parsing_shared::with_nested_meta,
    datastructure::{DataStructure, DataVariant, MyField, Struct},
    repr_attr::ReprAttr,
};

use proc_macro2::TokenStream as TokenStream2;

use syn::{Attribute, Lit, Meta, MetaList, MetaNameValue, WherePredicate};

use quote::ToTokens;

use std::marker::PhantomData;

////////////////////////////////////////////////////////////////////////////////

pub(crate) struct ZeroConfig<'a> {
    pub(crate) extra_predicates: Vec<WherePredicate>,

    /// The type parameters that don't have a `Zeroable` bound.
    pub(crate) unbounded_typarams: Vec<IsBounded>,

    /// Code that's inserted alongside the Zeroable assertions.
    /// Used in tests.
    pub(crate) test_code: Vec<TokenStream2>,

    /// If true,panics with the output of the derive macro.
    pub(crate) debug_print: bool,

    pub(crate) zeroable_fields: Vec<IsZeroable>,
    pub(crate) default_zeroab: IsZeroable,

    pub(crate) repr_attr: ReprAttr,

    _marker: PhantomData<&'a ()>,
}

impl<'a> ZeroConfig<'a> {
    fn new(za: ZeroableAttrs<'a>) -> Result<Self, syn::Error> {
        let ZeroableAttrs {
            extra_predicates,
            unbounded_typarams,
            test_code,
            debug_print,
            zeroable_fields,
            default_zeroab,
            repr_attr,
            _marker,
        } = za;

        Ok(Self {
            extra_predicates,
            unbounded_typarams,
            test_code,
            debug_print,
            zeroable_fields,
            default_zeroab,
            repr_attr,
            _marker,
        })
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum IsBounded {
    Yes,
    No,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum IsZeroable {
    Yes,
    No,
}

impl IsZeroable {
    pub(crate) fn new(val: bool) -> Self {
        match val {
            true => IsZeroable::Yes,
            false => IsZeroable::No,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

struct ZeroableAttrs<'a> {
    extra_predicates: Vec<WherePredicate>,
    unbounded_typarams: Vec<IsBounded>,
    test_code: Vec<TokenStream2>,
    debug_print: bool,
    zeroable_fields: Vec<IsZeroable>,
    default_zeroab: IsZeroable,
    repr_attr: ReprAttr,
    _marker: PhantomData<&'a ()>,
}

#[derive(Copy, Clone)]
#[allow(dead_code)]
enum ParseContext<'a> {
    TypeAttr { ds: &'a DataStructure<'a> },
    Variant { variant: &'a Struct<'a> },
    Field { field: &'a MyField<'a> },
}

pub(crate) fn parse_attrs_for_zeroed<'a>(
    ds: &'a DataStructure<'a>,
) -> Result<ZeroConfig<'a>, syn::Error> {
    let typaram_count = ds.generics.type_params().count();

    let mut this = ZeroableAttrs {
        extra_predicates: Vec::new(),
        unbounded_typarams: vec![IsBounded::Yes; typaram_count],
        test_code: Vec::new(),
        debug_print: false,
        zeroable_fields: if ds.data_variant == DataVariant::Union {
            vec![IsZeroable::Yes; ds.variants[0].fields.len()]
        } else {
            Vec::new()
        },
        default_zeroab: IsZeroable::Yes,
        repr_attr: ReprAttr::Rust,
        _marker: PhantomData,
    };

    parse_inner(&mut this, ds.attrs, ParseContext::TypeAttr { ds })?;

    for variant in &ds.variants {
        parse_inner(&mut this, variant.attrs, ParseContext::Variant { variant })?;
        for field in &variant.fields {
            parse_inner(&mut this, field.attrs, ParseContext::Field { field })?;
        }
    }

    ZeroConfig::new(this)
}

fn parse_inner<'a, I>(
    this: &mut ZeroableAttrs<'a>,
    attrs: I,
    pctx: ParseContext<'a>,
) -> Result<(), syn::Error>
where
    I: IntoIterator<Item = &'a Attribute>,
{
    for attr in attrs {
        match attr.parse_meta()? {
            Meta::List(list) => {
                parse_attr_list(this, list, pctx)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_attr_list<'a>(
    this: &mut ZeroableAttrs<'a>,
    list: MetaList,
    pctx: ParseContext<'a>,
) -> Result<(), syn::Error> {
    if list.path.is_ident("zero") {
        with_nested_meta("zero", list.nested, |attr| {
            parse_sabi_attr(this, pctx, attr)
        })?;
    } else if list.path.is_ident("repr") {
        this.repr_attr = ReprAttr::new(list.nested)?;
    }
    Ok(())
}

fn parse_sabi_attr<'a>(
    this: &mut ZeroableAttrs<'a>,
    pctx: ParseContext<'a>,
    attr: Meta,
) -> Result<(), syn::Error> {
    match (pctx, attr) {
        (
            ParseContext::TypeAttr { .. },
            Meta::NameValue(MetaNameValue {
                lit: Lit::Str(ref value),
                ref path,
                ..
            }),
        ) => {
            if path.is_ident("bound") {
                this.extra_predicates.push(value.parse()?);
            } else if path.is_ident("_test_code") {
                this.test_code.push(value.parse()?);
            } else {
                return_spanned_err! {path,"Unrecognized attribute"}
            }
        }
        (ParseContext::TypeAttr { ds }, Meta::List(list)) => {
            if list.path.is_ident("not_zeroable") {
                with_nested_meta("not_zeroable", list.nested, |attr| match &attr {
                    Meta::Path(path) => {
                        let tyident = path.get_ident().ok_or_else(|| {
                            spanned_err! {path,"Expected identifier for type parameter"}
                        })?;

                        let pos = ds
                            .generics
                            .type_params()
                            .position(|x| x.ident == *tyident)
                            .ok_or_else(|| spanned_err! {tyident,"Expected a type parameter"})?;

                        this.unbounded_typarams[pos] = IsBounded::No;

                        Ok(())
                    }
                    _ => {
                        return_spanned_err! {attr,"Expected identifier for type parameter"}
                    }
                })?;
            } else {
                return_spanned_err! {list,"Unrecognized attribute"}
            }
        }
        (ParseContext::TypeAttr { .. }, Meta::Path(path)) => {
            if path.is_ident("debug_print") {
                this.debug_print = true;
            } else if path.is_ident("nonzero_fields") {
                this.default_zeroab = IsZeroable::No;

                if this.zeroable_fields.is_empty() {
                    return_spanned_err! {
                        path,
                        "Cannot use the `#[zero(nonzero_fields)]` attribute on a struct/enum",
                    }
                }

                for zf in &mut this.zeroable_fields {
                    *zf = IsZeroable::No;
                }
            } else {
                return_spanned_err! {path,"Unrecognized attribute"}
            }
        }
        (ParseContext::Field { field }, Meta::Path(path)) => {
            let is_zeroable;

            if {
                is_zeroable = path.is_ident("zeroable");
                is_zeroable || path.is_ident("nonzero")
            } {
                match this.zeroable_fields.get_mut(field.index.pos) {
                    Some(zf) => *zf = IsZeroable::new(is_zeroable),
                    None => return_spanned_err! {
                        path,
                        "Cannot use the `#[zero({})]` attribute on a struct/enum field ",
                        path.to_token_stream(),
                    },
                }
            } else {
                return_spanned_err! {path,"Unrecognized attribute"}
            }
        }
        (_, attr) => {
            return_spanned_err! {attr,"Unrecognized attribute"}
        }
    }
    Ok(())
}

////////////////////////////////////////////////////////////////////////////////
