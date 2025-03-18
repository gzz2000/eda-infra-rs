//! compiler collection

use cc::Build;
use std::env;
use std::path::{ Path, PathBuf };

/// private util to add version definitions to the compiler.
fn add_definitions(builder: &mut Build) {
    macro_rules! add_definition {
        ($($def:ident),+) => {$(
            #[allow(non_snake_case)]
            let $def = env::var(stringify!($def)).ok();
            builder.define(stringify!($def),
                           $def.as_ref().map(|s| s.as_str()));
            println!("cargo:rerun-if-env-changed={}", stringify!($def));
        )+}
    }
    add_definition! {
        CARGO_PKG_NAME,
        CARGO_PKG_VERSION,
        CARGO_PKG_VERSION_MAJOR,
        CARGO_PKG_VERSION_MINOR,
        CARGO_PKG_VERSION_PATCH
    }
    builder.define("UCC_VERSION", env!("CARGO_PKG_VERSION"));
}

/// initialize a `cc` compiler with openmp support.
pub fn cl_cpp_openmp() -> Build {
    let mut builder = Build::new();
    println!("cargo:rerun-if-env-changed=CC");
    if cfg!(target_os = "macos") {
        if !env::var("CC").is_ok() {
            // if CC env var is not set, we try some common
            // overrides.
            if Path::new("/opt/homebrew/opt/llvm/bin/clang").exists() {
                // on macos m1, we use the homebrew clang compiler.
                // this supports openmp.
                //
                // this also supports some static linking of openmp,
                // but according to the openmp docs it is not recommended.
                builder.compiler("/opt/homebrew/opt/llvm/bin/clang");
                println!("cargo:rustc-link-search=/opt/homebrew/opt/llvm/lib");
                println!("cargo:rustc-link-search=/opt/homebrew/lib");
            }
        }
        // on macos, the library is omp.
        println!("cargo:rustc-link-lib=dylib=omp");
    }
    else {
        // on linux, the library is gomp.
        // static linking is also available but not very straightforward,
        // as the libgomp.a is hidden somewhere. also not preferred.
        println!("cargo:rustc-link-lib=dylib=gomp");
    }
    builder
        .cpp(true)
        .flag("-Wall")
        .flag("-fopenmp")
        .flag("-std=c++14")
        .out_dir(env::var_os("OUT_DIR").map(|v| {
            let mut v = PathBuf::from(v);
            v.push("ucc_cpp");
            v
        }).unwrap());
    add_definitions(&mut builder);
    builder
}

/// initialize a cuda compiler with given code generation options.
///
/// - if `gencode` is specified, gencode is applied to generate SASS code
///   for the specific capabilities.
/// - if `ptx_arch` is specified, arch is applied to attach PTX code
///   for at least the given capabilities.
///
/// it is ALWAYS suggested to specify `ptx_arch` as it makes the code
/// compatible to a wide range of GPU targets.
pub fn cl_cuda_arch(gencode: Option<&[u32]>, ptx_arch: Option<u32>) -> Build {
    let mut builder_cuda = Build::new();
    builder_cuda
        .cuda(true)
        .flag("-Xcompiler").flag("-Wall")
        .flag("-std=c++14");
    for arch in gencode.unwrap_or(&[]) {
        builder_cuda.flag("-gencode")
            .flag(&format!("arch=compute_{arch},code=sm_{arch}"));
    }
    if let Some(ptx_arch) = ptx_arch {
        builder_cuda.flag(&format!("-arch=compute_{ptx_arch}"));
        builder_cuda.flag(&format!("-code=sm_{ptx_arch},compute_{ptx_arch}"));
    }
    builder_cuda.out_dir(env::var_os("OUT_DIR").map(|v| {
        let mut v = PathBuf::from(v);
        v.push("ucc_cuda");
        v
    }).unwrap());
    add_definitions(&mut builder_cuda);
    builder_cuda
}

/// a shorthand for frequently-used cuda compiler options, can be controlled
/// by environment variables.
///
/// if nothing is given in environment variables, we will generate PTX for
/// cc5.0, and SASS for cc8.0 and cc7.0.
///
/// if `UCC_CUDA_PTX` is set, ptx is set to the given version (empty means
/// no ptx is generated).
/// if `UCC_CUDA_GENCODE` is set, gencode is set to given versions (comma
/// separated).
///
/// if you want direct control, see [`cl_cuda_arch`].
pub fn cl_cuda() -> Build {
    println!("cargo:rerun-if-env-changed=UCC_CUDA_PTX");
    println!("cargo:rerun-if-env-changed=UCC_CUDA_GENCODE");
    let ptx_arch = match env::var("UCC_CUDA_PTX") {
        Ok(v) => if v.is_empty() { None } else { Some(v.parse().unwrap()) },
        Err(_) => Some(50)
    };
    let gencode = match env::var("UCC_CUDA_GENCODE") {
        Ok(v) => if v.is_empty() { None } else { Some(
            v.split(',').map(|i| i.parse().unwrap()).collect::<Vec<_>>()
        ) },
        Err(_) => Some(vec![80, 70])
    };
    cl_cuda_arch(gencode.as_deref(), ptx_arch)
}
