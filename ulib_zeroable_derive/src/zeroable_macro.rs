use crate::{
    datastructure::{DataStructure, DataVariant, MyField},
    repr_attr::ReprAttr,
    utils::ExprExt,
};

use proc_macro2::TokenStream as TokenStream2;

use quote::{quote, quote_spanned};

use syn::{punctuated::Punctuated, DeriveInput};

mod attribute_parsing;

#[cfg(test)]
mod tests;

use self::attribute_parsing::{IsBounded, IsZeroable, ZeroConfig};

pub fn derive(ref data: DeriveInput) -> Result<TokenStream2, syn::Error> {
    let ds = &DataStructure::new(data);

    let config = &attribute_parsing::parse_attrs_for_zeroed(ds)?;

    let field_asserts = match ds.data_variant {
        DataVariant::Struct => emit_field_assertions(&ds.variants[0].fields),
        DataVariant::Enum => checks_and_emit_enum_field_assertions(ds, config)?,
        DataVariant::Union => checks_and_emit_union_field_assertions(ds, config)?,
    };

    let zeroable_docs = match ds.data_variant {
        _ if !ds.is_public() => String::new(),
        DataVariant::Struct => String::new(),
        DataVariant::Enum => String::new(),
        DataVariant::Union => docs_for_union(ds, config),
    };

    let name = ds.name;

    let mut unbounded_tp = config.unbounded_typarams.iter().cloned();
    let ty_params = ds
        .generics
        .type_params()
        .map(|x| &x.ident)
        .filter(move |_| unbounded_tp.next() != Some(IsBounded::No));

    let extra_predicates = &*config.extra_predicates;

    let (impl_generics, ty_generics, where_clause) = ds.generics.split_for_impl();

    let empty_preds = Punctuated::new();

    let where_preds = where_clause
        .as_ref()
        .map_or(&empty_preds, |x| &x.predicates)
        .into_iter();

    let where_clause_tokens = quote!(
        where
            #( #where_preds ,)*
            #( #ty_params: ::ulib::Zeroable, )*
            #( #extra_predicates ,)*
    );

    let test_code = &*config.test_code;

    let tokens = quote!(
        #[doc(hidden)]
        impl #impl_generics #name #ty_generics
        #where_clause_tokens
        {
            const _ASSERT_IS_ZEROABLE_101:()={
                #({ #test_code })*
                const fn assert_impl_zeroable<T: ::ulib::Zeroable>() {}
                #field_asserts
            };
        }

        #[doc=#zeroable_docs]
        unsafe impl #impl_generics ::ulib::Zeroable for #name #ty_generics
        #where_clause_tokens
        {}
    );

    if config.debug_print {
        panic!("\n\n\n{}\n\n\n", tokens);
    }

    Ok(tokens)
}

fn checks_and_emit_enum_field_assertions(
    ds: &'_ DataStructure<'_>,
    config: &'_ ZeroConfig<'_>,
) -> Result<TokenStream2, syn::Error> {
    assert_eq!(ds.data_variant, DataVariant::Enum);

    const REPR_ERR: &str = "Expected a `#[repr(C/<integer_type>/Transparent)]`enum";

    const ENUM_DISCR_ERR: &str = "
Expected either:

- The first variant to have an implicit discriminant,

- Any variant with an explicit `0` discriminant,
    ";

    if ds.variants.len() == 0 {
        return_spanned_err! { ds.name,"Zero variant enums cannot implement Zeroable." }
    }

    let zero_variant = match config.repr_attr {
        ReprAttr::C { .. } | ReprAttr::IntegerRepr { .. } => {
            if ds.variants[0].discriminant.map_or(true, ExprExt::is_zero) {
                0
            } else {
                let offset = 1;
                ds.variants[offset..]
                    .iter()
                    .position(|var| var.discriminant.map_or(false, ExprExt::is_zero))
                    .ok_or_else(move || spanned_err! { ds.name,"{}",ENUM_DISCR_ERR })?
                    + offset
            }
        }
        ReprAttr::Transparent => 0,
        ReprAttr::Rust => {
            return_spanned_err! { ds.name,"{}",REPR_ERR }
        }
    };

    Ok(emit_field_assertions(&ds.variants[zero_variant].fields))
}

fn checks_and_emit_union_field_assertions(
    ds: &'_ DataStructure<'_>,
    config: &'_ ZeroConfig<'_>,
) -> Result<TokenStream2, syn::Error> {
    assert_eq!(ds.data_variant, DataVariant::Union);

    const EXPECTED_NONZERO: &str =
        "Expected one or more union fields without a `#[zero(nonzero)]` attribute.";

    const EXPECTED_ZEROABLE: &str =
        "Expected one or more union fields with a `#[zero(zeroable)]` attribute.";

    let union_ = &ds.variants[0];

    if union_.fields.len() == 0 {
        return_spanned_err! { ds.name,"Zero fields union cannot implement Zeroable." }
    } else {
        let zeroable_fields = if config.repr_attr == ReprAttr::Transparent {
            vec![&union_.fields[0]]
        } else {
            config
                .zeroable_fields
                .iter()
                .cloned()
                .zip(&union_.fields)
                .filter_map(|(zeroableness, field)| match zeroableness {
                    IsZeroable::No => None,
                    IsZeroable::Yes => Some(field),
                })
                .collect::<Vec<_>>()
        };

        match (zeroable_fields.is_empty(), config.default_zeroab) {
            (true, IsZeroable::Yes) => return_spanned_err!(ds.name, "{}", EXPECTED_NONZERO),
            (true, IsZeroable::No) => return_spanned_err!(ds.name, "{}", EXPECTED_ZEROABLE),
            (false, _) => {}
        }

        Ok(emit_field_assertions(zeroable_fields))
    }
}

fn emit_field_assertions<'a, I>(fields: I) -> TokenStream2
where
    I: IntoIterator<Item = &'a MyField<'a>>,
{
    fields
        .into_iter()
        .map(|field| {
            let ty = field.ty;
            quote_spanned!(field.ty_span()=>
                { assert_impl_zeroable::<#ty>(); }
            )
        })
        .collect()
}

fn docs_for_union(ds: &'_ DataStructure<'_>, config: &'_ ZeroConfig<'_>) -> String {
    use quote::ToTokens;

    use std::fmt::Write;

    assert_eq!(ds.data_variant, DataVariant::Union);

    let union_ = &ds.variants[0];

    let mut buffer = String::with_capacity(256);

    let mut zeroable_fields = Vec::new();
    let mut nonzero_fields = Vec::new();

    config
        .zeroable_fields
        .iter()
        .zip(&union_.fields)
        .filter(|(_, f)| f.is_public())
        .for_each(|(zeroableness, field)| {
            match *zeroableness {
                IsZeroable::No => &mut nonzero_fields,
                IsZeroable::Yes => &mut zeroable_fields,
            }
            .push(field)
        });

    fn output_fields(buffer: &mut String, fields: Vec<&MyField<'_>>) {
        for field in fields {
            let ty = field.ty.to_token_stream();
            let _ = write!(buffer, "- `{}: {}` \n\n", field.ident(), ty);
        }
    }

    if zeroable_fields.len() + nonzero_fields.len() < union_.fields.len() {
        buffer.push_str("# Private Fields\n\n");
        buffer.push_str("Private fields are omitted in this documentation.\n\n");
    }

    if !zeroable_fields.is_empty() {
        buffer.push_str("# Zeroable Fields\n\n");
        buffer.push_str(
            "These fields can be safely accessed in the return value of `Self::zeroed()`:\n\n",
        );
        output_fields(&mut buffer, zeroable_fields);
    }

    if !nonzero_fields.is_empty() {
        buffer.push_str("# NonZero Fields\n\n");
        buffer.push_str(
            "These fields may be unsound to access in the return value of `Self::zeroed()`:\n\n",
        );

        output_fields(&mut buffer, nonzero_fields);
    }

    buffer
}
