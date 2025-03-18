//! this build script exports the csrc dir to dependents.

fn main() {
    println!("Building the C source files of ulib..");
    println!("cargo:rerun-if-changed=csrc");

    let mut cl_cpp = ucc::cl_cpp_openmp();
    cl_cpp.file("csrc/memfill.cpp");
    cl_cpp.compile("ulibc");
    println!("cargo:rustc-link-lib=static=ulibc");

    #[cfg(feature = "cuda")]
    let cl_cuda = {
        let mut cl_cuda = ucc::cl_cuda();
        cl_cuda.ccbin(false);
        cl_cuda.debug(false).opt_level(3).file("csrc/memfill.cu");
        cl_cuda.compile("ulibcu");
        println!("cargo:rustc-link-lib=static=ulibcu");
        println!("cargo:rustc-link-lib=dylib=cudart");
        cl_cuda
    };

    ucc::bindgen([
        "csrc/memfill.cpp",
        #[cfg(feature = "cuda")] "csrc/memfill.cu"
    ], "memfill.rs");

    ucc::export_csrc();
    ucc::make_compile_commands(&[
        &cl_cpp,
        #[cfg(feature = "cuda")] &cl_cuda
    ]);
}
