use quote::ToTokens;

use syn::{Expr, ExprLit, Lit, LitInt};

use std::fmt::Display;

///////////////////////////////////////////////////////////////////////////////

pub(crate) fn spanned_err(tokens: &dyn ToTokens, display: &dyn Display) -> syn::Error {
    syn::Error::new_spanned(tokens, display)
}

///////////////////////////////////////////////////////////////////////////////

pub(crate) trait ExprExt {
    fn is_zero(&self) -> bool;
}

impl ExprExt for Expr {
    fn is_zero(&self) -> bool {
        match self {
            Expr::Lit(ExprLit {
                lit: Lit::Int(integer),
                ..
            }) => integer.is_zero(),
            _ => false,
        }
    }
}

impl ExprExt for LitInt {
    fn is_zero(&self) -> bool {
        self.base10_digits() == "0"
    }
}
