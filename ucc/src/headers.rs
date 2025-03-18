//! implementation of header import/exports.

use std::fs;
use std::env;
use std::path::{ Path, PathBuf };
use std::time::SystemTime;

/// This exports the `csrc` directory to the environment variable,
/// so that it can be found and included by crates that depend
/// on the current crate.
///
/// This relies on a `links` property to be set in `Cargo.toml`.
/// See [the cargo docs](https://doc.rust-lang.org/cargo/reference/build-scripts.html#the-links-manifest-key) for more info.
/// In theory, the value of the `links` property does not matter
/// as long as it is distinct across crates. It is recommended
/// to just use the crate name.
pub fn export_csrc() {
    println!("cargo:rerun-if-changed=csrc");
    println!("cargo:rerun-if-changed=Cargo.toml");
    match env::var("CARGO_MANIFEST_LINKS").as_ref().map(|s| s.as_ref()) {
        Err(_) | Ok("") => panic!(
            "You need to set a links property in your Cargo.toml \
             to export csrc."),
        _ => {}
    }
    println!("cargo:ucc_csrc_exported=1");
    // we will always use pkg name in includes dirs, regardless
    // of the links property.
    println!("cargo:ucc_csrc_pkg_name={}",
             env::var("CARGO_PKG_NAME").unwrap());
    // for detecting dependency rebuild.
    println!("cargo:ucc_csrc_build_time={}",
             SystemTime::now()
             .duration_since(SystemTime::UNIX_EPOCH).unwrap()
             .as_nanos());
    println!("cargo:ucc_csrc_manifest_dir={}",
             env::var("CARGO_MANIFEST_DIR").unwrap());
    println!("cargo:ucc_csrc_out_dir={}",
             env::var("OUT_DIR").unwrap());
}

/// This imports the `csrc` directory of all direct dependency
/// crates that were exported using [`export_csrc`],
/// save their files in a temp dir inside `OUT_DIR`, and return
/// the path of that dir.
/// If that dir is used as include search path, you can then
/// use `#include <crates/{crate name}/xxx.hpp>` to include
/// the `csrc/xxx.hpp` file in the dependency.
#[must_use]
pub fn import_csrc() -> PathBuf {
    println!("cargo:rerun-if-changed=csrc");
    let out_dir = Path::new(&env::var("OUT_DIR").unwrap())
        .join("ucc_csrc_includes");
    let out_crates_dir = out_dir.join("crates");
    drop(fs::remove_dir_all(&out_dir));
    fs::create_dir_all(&out_crates_dir).unwrap();
    for (k, _v) in env::vars() {
        if !(k.starts_with("DEP_") && k.ends_with("_UCC_CSRC_EXPORTED")) {
            continue
        }
        let pkg = &k[..k.len() - 18];
        println!("cargo:rerun-if-env-changed={}_UCC_CSRC_BUILD_TIME", pkg);
        let gvar = |key| env::var(format!("{}_UCC_CSRC_{}", pkg, key)).unwrap();
        let manifest_dir = PathBuf::from(gvar("MANIFEST_DIR"));
        let pkg_name = gvar("PKG_NAME");
        let csrc_dir = manifest_dir.join("csrc");
        let incl_dir = out_crates_dir.join(&pkg_name);
        println!("[ucc csrc] {} -> {}",
                 csrc_dir.display(), incl_dir.display());

        #[cfg(unix)]
        std::os::unix::fs::symlink(&csrc_dir, &incl_dir).unwrap();
        #[cfg(not(unix))]
        fs_extra::copy_items(
            &[&csrc_dir], &incl_dir,
            &fs_extra::dir::CopyOptions::new()
                .copy_inside(true)).unwrap();
    }
    out_dir
}
