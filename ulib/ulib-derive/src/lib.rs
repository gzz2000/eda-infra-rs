//! Ulib derive macros.
//!
//! This implements the derive macro of `ulib::UniversalCopy`.
//! It is adapted from [the source code of `cust_derive`](https://docs.rs/cust_derive/latest/src/cust_derive/lib.rs.html#20-24).

use proc_macro2::{Ident, Span, TokenStream};
use syn::{
    parse_str, Data, DataEnum, DataStruct, DataUnion, DeriveInput, Field, Fields, Generics,
    TypeParamBound,
};
use quote::quote;

#[proc_macro_derive(UniversalCopy)]
pub fn universal_copy(input: BaseTokenStream) -> BaseTokenStream {
    let ast = syn::parse(input).unwrap();
    let gen = impl_universal_copy(&ast);
    BaseTokenStream::from(gen)
}

use proc_macro::TokenStream as BaseTokenStream;

fn impl_universal_copy(input: &DeriveInput) -> TokenStream {
    let input_type = &input.ident;

    let check_types_code = match input.data {
        Data::Struct(ref data_struct) => type_check_struct(data_struct),
        Data::Enum(ref data_enum) => type_check_enum(data_enum),
        Data::Union(ref data_union) => type_check_union(data_union),
    };

    let type_test_func_name = format!(
        "__ulib_derive_verify_{}_can_implement_universalcopy",
        input_type.to_string().to_lowercase()
    );
    let type_test_func_ident = Ident::new(&type_test_func_name, Span::call_site());

    // If the struct/enum/union is generic, we need to add the DeviceCopy bound to the generics
    // when implementing DeviceCopy.
    let generics = add_bound_to_generics(&input.generics, quote! {
        ::std::marker::Copy
    });
    #[cfg(feature = "cuda")]
    let generics = add_bound_to_generics(&generics, quote! {
        ::std::marker::Copy
    });
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    // Finally, generate the unsafe impl and the type-checking function.
    #[cfg(feature = "cuda")]
    let impl_cuda = quote! {
        unsafe impl #impl_generics ::ulib::cust::memory::DeviceCopy for #input_type #type_generics #where_clause {}
    };
    #[cfg(not(feature = "cuda"))]
    let impl_cuda = quote! {};
    
    #[cfg(feature = "cuda")]
    let trait_bounds = quote! {
        ::std::marker::Copy + ::ulib::cust::memory::DeviceCopy
    };
    #[cfg(not(feature = "cuda"))]
    let trait_bounds = quote! {
        ::std::marker::Copy
    };
    
    let generated_code = quote! {
        impl #impl_generics ::std::marker::Copy for #input_type #type_generics #where_clause {}
        #impl_cuda

        #[doc(hidden)]
        #[allow(all)]
        fn #type_test_func_ident #impl_generics(value: & #input_type #type_generics) #where_clause {
            fn assert_impl<T: #trait_bounds>() {}
            #check_types_code
        }
    };

    generated_code
}

fn add_bound_to_generics(generics: &Generics, import: TokenStream) -> Generics {
    let mut new_generics = generics.clone();
    let bound: TypeParamBound = parse_str(&quote! {#import}.to_string()).unwrap();

    for type_param in &mut new_generics.type_params_mut() {
        type_param.bounds.push(bound.clone())
    }

    new_generics
}

fn type_check_struct(s: &DataStruct) -> TokenStream {
    let checks = match s.fields {
        Fields::Named(ref named_fields) => {
            let fields: Vec<&Field> = named_fields.named.iter().collect();
            check_fields(&fields)
        }
        Fields::Unnamed(ref unnamed_fields) => {
            let fields: Vec<&Field> = unnamed_fields.unnamed.iter().collect();
            check_fields(&fields)
        }
        Fields::Unit => vec![],
    };
    quote!(
        #(#checks)*
    )
}

fn type_check_enum(s: &DataEnum) -> TokenStream {
    let mut checks = vec![];

    for variant in &s.variants {
        match variant.fields {
            Fields::Named(ref named_fields) => {
                let fields: Vec<&Field> = named_fields.named.iter().collect();
                checks.extend(check_fields(&fields));
            }
            Fields::Unnamed(ref unnamed_fields) => {
                let fields: Vec<&Field> = unnamed_fields.unnamed.iter().collect();
                checks.extend(check_fields(&fields));
            }
            Fields::Unit => {}
        }
    }
    quote!(
        #(#checks)*
    )
}

fn type_check_union(s: &DataUnion) -> TokenStream {
    let fields: Vec<&Field> = s.fields.named.iter().collect();
    let checks = check_fields(&fields);
    quote!(
        #(#checks)*
    )
}

fn check_fields(fields: &[&Field]) -> Vec<TokenStream> {
    fields
        .iter()
        .map(|field| {
            let field_type = &field.ty;
            quote! {assert_impl::<#field_type>();}
        })
        .collect()
}
