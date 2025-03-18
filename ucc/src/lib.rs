//! The universal compiler invocation library for Rust-CXX-CUDA
//! interoperation and dependency management.
//!
//! This crate serves as a universal bridge between Rust code
//! and heterogeneous C++/CUDA/.. codes. It does the following
//! work:
//!
//! * Manages and exports/imports the C++ headers between
//!   crates, through [`export_csrc`].
//!   This feature makes use of the Cargo dependency management
//!   framework to conveniently manage C++/CUDA code
//!   dependencies.
//!
//! * Compile C++(OpenMP)/CUDA sources with out-of-the-box
//!   compilation settings and common platform detections.
//!   
//! * Generates bindings of C++/CUDA host functions
//!   making use of `ulib::UVec`, through [`bindgen()`].
//!
//! * Generates `compile_commands.json` for language servers,
//!   automatically writes it to project root and takes care
//!   of dependency sources as well.

mod bindgen;
pub use bindgen::bindgen;

mod headers;
pub use headers::{ export_csrc, import_csrc };

mod compile;
pub use compile::{ cl_cpp_openmp, cl_cuda, cl_cuda_arch };

mod lsp;
pub use lsp::make_compile_commands;
