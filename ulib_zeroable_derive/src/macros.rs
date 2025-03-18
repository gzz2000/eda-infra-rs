macro_rules! spanned_err {
    ( $e:expr, $($fmt:tt)* ) => ({
        crate::utils::spanned_err(
            &$e,
            &format!($($fmt)*),
        )
    })
}

macro_rules! return_spanned_err {
    ( $e:expr, $($fmt:tt)* ) => ({
        return Err(crate::utils::spanned_err(
            &$e,
            &format!($($fmt)*),
        ))
    })
}
