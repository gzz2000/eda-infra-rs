use crate::attribute_parsing_shared::with_nested_meta;

use quote::ToTokens;

use syn::{Meta, NestedMeta};

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Copy, Clone, PartialEq)]
#[allow(dead_code)]
pub(crate) enum ReprAttr {
    C { integer_repr: bool },
    IntegerRepr,
    Transparent,
    Rust,
}

#[derive(Copy, Clone)]
pub(crate) struct ReprAttrBuilder {
    c: bool,
    integer: bool,
    transparent: bool,
}

impl ReprAttr {
    pub fn new<I>(iter: I) -> Result<Self, syn::Error>
    where
        I: IntoIterator<Item = NestedMeta>,
    {
        const REPR_RUST: ReprAttrBuilder = ReprAttrBuilder {
            c: false,
            integer: false,
            transparent: false,
        };

        let mut this = REPR_RUST;

        fn inner_err(tokens: &dyn ToTokens) -> syn::Error {
            spanned_err!(tokens, "Unrecognized repr attribute")
        }

        with_nested_meta("repr", iter, |attr| match attr {
            Meta::Path(ref path) => {
                let ident = path.get_ident().ok_or_else(|| inner_err(path))?;

                if ident == "C" {
                    this.c = true;
                } else if ident == "transparent" {
                    this.transparent = true;
                } else if is_integer_type(ident) {
                    this.integer = true;
                } else {
                    return Err(inner_err(ident));
                }
                Ok(())
            }
            Meta::List(ref list) if list.path.is_ident("align") => Ok(()),
            x => Err(inner_err(&x)),
        })?;

        // Ignoring these invalid combinations because Rust already
        // emits an error for them:
        //  - #[repr(transparent,C)]
        //  - #[repr(transparent,<integer_type>)]
        match (this.c, this.integer, this.transparent) {
            (true, integer_repr, _) => Ok(ReprAttr::C { integer_repr }),
            (false, true, _) => Ok(ReprAttr::IntegerRepr),
            (false, false, true) => Ok(ReprAttr::Transparent),
            (false, false, false) => Ok(ReprAttr::Rust),
        }
    }
}

macro_rules! matches_one_integer_repr {
    ( $matches:ident => $( $repr:expr ),* $(,)* ) => (
        match () {
            $(() if $matches == $repr => true,)*
            _=>false
        }
    )
}

fn is_integer_type(ident: &syn::Ident) -> bool {
    matches_one_integer_repr! {
        ident=>
        "u8","i8",
        "u16","i16",
        "u32","i32",
        "u64","i64",
        "u128","i128",
        "usize","isize",

    }
}
