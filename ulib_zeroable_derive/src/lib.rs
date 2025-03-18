//! An implementation detail of `zeroable`.

#![deny(unreachable_patterns)]
#![deny(unused_doc_comments)]
#![deny(unconditional_recursion)]

extern crate proc_macro;

#[macro_use]
mod macros;

mod attribute_parsing_shared;
mod datastructure;
mod repr_attr;
mod utils;
mod zeroable_macro;

use proc_macro::TokenStream as TokenStream1;
use proc_macro2::TokenStream as TokenStream2;

/// This macro is documented in [`zeroable::zeroable_docs`](./zeroable_docs/index.html).
/// (the link only works in the `zeroable` crate)
#[proc_macro_derive(Zeroable, attributes(zero))]
pub fn derive_zeroable(input: TokenStream1) -> TokenStream1 {
    parse_or_compile_err(input, zeroable_macro::derive).into()
}

////////////////////////////////////////////////////////////////////////////////

fn parse_or_compile_err<P, F>(input: TokenStream1, f: F) -> TokenStream2
where
    P: syn::parse::Parse,
    F: FnOnce(P) -> Result<TokenStream2, syn::Error>,
{
    syn::parse::<P>(input)
        .and_then(f)
        .unwrap_or_else(|e| e.to_compile_error())
}
