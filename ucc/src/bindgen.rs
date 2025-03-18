//! binding generator

use std::fs;
use std::env;
use std::path::{ Path, PathBuf };
use tree_sitter::Parser;
use proc_macro2::{ TokenStream, Ident, Literal, Span };

#[cfg(feature = "ulib")]
use indexmap::IndexMap;

#[derive(Debug, PartialEq, Eq)]
struct ArraySize(String);

impl quote::ToTokens for ArraySize {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use quote::TokenStreamExt;
        match self.0.parse::<usize>() {
            Ok(v) => tokens.append(Literal::usize_suffixed(v)),
            Err(_) => tokens.append(Ident::new(&self.0, Span::call_site()))
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum FnParamType {
    /// e.g.: `usize n` -> `Scalar, "usize"`
    Scalar,
    /// e.g.:
    /// `const Object *objs` -> `ConstList, "Object"`
    ConstList,
    /// e.g.:
    /// `Object *objs` -> `MutList, "Object"`
    MutList,
    /// e.g.:
    /// `const Object (*objarr)[2] -> `ConstListArray(2), "Object"`
    ConstListArray(ArraySize),
    /// e.g.:
    /// `Object (*objarr)[2] -> `MutListArray(2), "Object"`
    MutListArray(ArraySize),
}

#[derive(Debug)]
struct FnSig {
    raw_name: String,
    param_names: Vec<String>,
    param_types: Vec<(FnParamType, String)>,
}

fn parse_insert_fns(source: &str, fns: &mut Vec<FnSig>) -> Option<()> {
    let mut parser = Parser::new();
    parser.set_language(tree_sitter_cpp::language())
        .expect("Error loading CPP grammar");
    let parsed = parser.parse(source, None)?;
    let cpp = parsed.root_node();
    
    for node in cpp.children(&mut cpp.walk()) {
        macro_rules! skip_ifn {
            ($v:expr) => { if !$v { continue } }
        }
        skip_ifn!(node.kind() == "linkage_specification");
        skip_ifn!(&source[node.child_by_field_name("value")?
                          .byte_range()] == "\"C\"");
        
        let node = node.child_by_field_name("body")?;
        skip_ifn!(node.kind() == "function_definition");
        skip_ifn!(&source[node.child_by_field_name("type")?
                          .byte_range()] == "void");

        let node = node.child_by_field_name("declarator")?;
        let raw_name = String::from(
            &source[node.child_by_field_name("declarator")?.byte_range()]
        );
        
        let params = node.child_by_field_name("parameters")?;
        let (param_names, param_types) = params.children(&mut params.walk()).filter_map(|param| {
            macro_rules! skip_ifn {
                ($v:expr) => { if !$v { return None } }
            }
            skip_ifn!(param.kind() == "parameter_declaration");
            let typ = String::from(
                &source[param.child_by_field_name("type")?.byte_range()]
            );
            let isconst = match param.child(0) {
                Some(c) if c.kind() == "type_qualifier" &&
                    &source[c.byte_range()] == "const" => true,
                _ => false
            };
            let decl = param.child_by_field_name("declarator")?;
            use FnParamType::*;
            let (name, fnparam) = match decl.kind() {
                "pointer_declarator" => (
                    decl.child_by_field_name("declarator")?.byte_range(),
                    match isconst {
                        true => ConstList,
                        false => MutList
                    }
                ),
                "identifier" => (decl.byte_range(), Scalar),
                "array_declarator" => {
                    let size = ArraySize(String::from(
                        &source[decl.child_by_field_name("size")?
                                .byte_range()]
                    ));
                    let inner_decl = decl.child_by_field_name("declarator")?;
                    skip_ifn!(inner_decl.kind() == "parenthesized_declarator" &&
                              inner_decl.child_count() == 3);
                    let chld = inner_decl.child(1)?;
                    skip_ifn!(chld.kind() == "pointer_declarator");
                    let name = chld.child_by_field_name("declarator")?.byte_range();
                    (name, match isconst {
                        true => ConstListArray(size),
                        false => MutListArray(size)
                    })
                },
                _ => return None
            };
            let name = String::from(&source[name]);
            Some((name, (fnparam, typ)))
        }).unzip();
        
        fns.push(FnSig {
            raw_name, param_names, param_types
        })
    }
    Some(())
}

/// Generate universal bindings for `export "C"` functions inside a
/// specified c++/cuda source, and write that binding to a Rust source
/// under `OUT_DIR/uccbind`.
/// 
/// Usage:
/// ```no_run
/// ucc::bindgen(["csrc/source.cpp"], "source.rs");
/// ```
/// You can then use that source in your Rust program like this:
/// ```ignore
/// mod ucci {
///     include!(concat!(env!("OUT_DIR"), "/uccbind/source.rs"));
/// }
/// ```
///
/// This generates `AsRef/AsMut` for cpu bindings,
/// and `AsURef/AsUMut` for cpu/gpu bindings.
/// the functionality is guessed by the suffix of function name:
/// `xxxx_cpu` or `xxxx_cuda`.
pub fn bindgen(source_list: impl IntoIterator<Item = impl AsRef<Path>>,
               dest: impl AsRef<Path>) {
    let root_dir = PathBuf::from(&env::var("CARGO_MANIFEST_DIR").unwrap());

    let mut fns = vec![];
    for source_file in source_list {
        let source = fs::read_to_string(
            root_dir.join(source_file.as_ref())
        ).unwrap();
        let source_clean = source.replace("__restrict__", "");
        parse_insert_fns(&source_clean, &mut fns);
    }
    // panic!("{:#?}", fns)

    use quote::*;
    use FnParamType::*;

    fn format_params(
        f: &FnSig,
        typmap: impl Fn(&Ident, &Ident, &FnParamType) -> TokenStream
    ) -> Vec<TokenStream> {
        f.param_names.iter().zip(f.param_types.iter())
            .map(|(name, (typ, typname))| {
                let pname = format_ident!("{}", name);
                let typname = format_ident!("{}", typname);
                typmap(&pname, &typname, typ)
            })
            .collect()
    }
    
    let ffis = fns.iter().map(|f| {
        let fname = format_ident!("{}", f.raw_name);
        let params = format_params(f, |p, typname, t| match t {
            Scalar => quote!{ #p: #typname },
            ConstList => quote!{ #p: *const #typname },
            MutList => quote!{ #p: *mut #typname },
            ConstListArray(size) => quote!{ #p: *const [#typname; #size] },
            MutListArray(size) => quote!{ #p: *mut [#typname; #size] },
        });
        quote!{
            pub fn #fname(#(#params),*);
        }
    });
    
    let mut funcdefs = vec![];
    for f in &fns {
        // build raw cpu-only bindings.
        if !f.raw_name.ends_with("_cuda") {
            let fname = format_ident!("{}", f.raw_name);
            let params = format_params(f, |p, typname, t| match t {
                Scalar => quote!{ #p: #typname },
                ConstList => quote!{ #p: impl AsRef<[#typname]> },
                MutList => quote!{ mut #p: impl AsMut<[#typname]> },
                ConstListArray(sz) => quote!{ #p: impl AsRef<[[#typname; #sz]]> },
                MutListArray(sz) => quote!{ mut #p: impl AsMut<[[#typname; #sz]]> },
            });
            let calls = format_params(f, |p, _typname, t| match t {
                Scalar => quote!{ #p },
                ConstList | ConstListArray(_) => quote!{ #p.as_ref().as_ptr() },
                MutList | MutListArray(_) => quote!{ #p.as_mut().as_mut_ptr() },
            });
            funcdefs.push(quote!{
                pub fn #fname(#(#params),*) {
                    unsafe { ffi::#fname(#(#calls),*) }
                }
            });
        }
    }
    
    #[cfg(feature = "ulib")]
    let mut ufuncs: IndexMap<&str, Vec<&FnSig>>
        = IndexMap::new();
    #[cfg(feature = "ulib")]
    for f in &fns {
        // record universal func signatures
        if f.raw_name.ends_with("_cpu") {
            ufuncs.entry(&f.raw_name[..f.raw_name.len() - 4])
                .or_default()
                .push(f);
        }
        else if f.raw_name.ends_with("_cuda") {
            ufuncs.entry(&f.raw_name[..f.raw_name.len() - 5])
                .or_default()
                .push(f);
        }
    }
    #[cfg(feature = "ulib")]
    for (uf, impls) in ufuncs {
        let f0 = &impls[0];
        if impls.len() > 1 && impls[1..].iter()
            .any(|i| i.param_types != f0.param_types)
        {
            println!("cargo:warning=ucc found incompatible function signatures in universal functions {:?}, skipping it.",
                     impls.iter().map(|i| &i.raw_name)
                     .collect::<Vec<_>>());
            continue;
        }
        
        let fname = format_ident!("{}", uf);
        let params = format_params(f0, |p, typname, t| match t {
            Scalar => quote!{ #p: #typname },
            ConstList => quote!{ #p: impl ulib::AsUPtr<#typname> },
            MutList => quote!{ mut #p: impl ulib::AsUPtrMut<#typname> },
            ConstListArray(sz) => quote!{ #p: impl ulib::AsUPtr<[#typname; #sz]> },
            MutListArray(sz) => quote!{ mut #p: impl ulib::AsUPtrMut<[#typname; #sz]> },
        });
        let calls = format_params(f0, |p, _typname, t| match t {
            Scalar => quote!{ #p },
            ConstList | ConstListArray(_) => quote!{ #p.as_uptr(device) },
            MutList | MutListArray(_) => quote!{ #p.as_mut_uptr(device) },
        });
        
        let devs = impls.iter().map(|f| {
            let fname = format_ident!("{}", f.raw_name);
            if f.raw_name.ends_with("_cpu") {
                quote!{
                    ulib::Device::CPU => {
                        unsafe { ffi::#fname(#(#calls),*) }
                    }
                }
            }
            else if f.raw_name.ends_with("_cuda") {
                quote!{
                    ulib::Device::CUDA(_devid) => {
                        let _context = device.get_context();
                        unsafe { ffi::#fname(#(#calls),*) }
                    }
                }
            }
            else { unreachable!("{}", f.raw_name) }
        });
        
        funcdefs.push(quote!{
            pub fn #fname(#(#params,)* device: ulib::Device) {
                match device {
                    #(#devs,)*
                    _ => panic!("unsupported device {:?}", device)
                }
            }
        });
    }
    
    let binds = quote!{
        #[allow(unused_imports)]
        use super::*; // import types
        #[allow(dead_code, unreachable_patterns, non_snake_case)]
        pub mod ffi {
            #[allow(unused_imports)]
            use super::*;
            extern "C" {
                #(#ffis)*
            }
        }
        #(
            #[allow(dead_code, unreachable_patterns, non_snake_case)]
            #funcdefs
        )*
    };
    let binds = prettyplease::unparse(&syn::parse2(binds).unwrap());
    // panic!("{}", binds)

    let dest_file = Path::new(&env::var("OUT_DIR").unwrap())
        .join("uccbind").join(dest);
    let dest_dir = dest_file.parent().unwrap();
    fs::create_dir_all(&dest_dir).unwrap();
    fs::write(&dest_file, binds).unwrap();
}
