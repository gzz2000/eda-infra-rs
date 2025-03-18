//! Writes out compile commands for CXX language servers.

use cc::Build;
use std::collections::HashSet;
use std::env;
use std::fs::{ self, File };
use std::io::BufReader;
use std::path::PathBuf;
use serde::{ Serialize, Deserialize };

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
struct CommandRecord {
    directory: String,
    command: String,
    file: String
}

/// create `compile_commands.json` for language servers like `clangd`.
/// this is very useful if you want to use code completion features.
///
/// this combines the settings of multiple `Build`s to one
/// `compile_commands.json` file, and write it out to env `OUT_DIR`.
///
/// the result will include sources of all dependencies that also use
/// `ucc` and also exported their compile commands.
///
/// if the package contains git tracking, this also put a copy of
/// `compile_commands.json` at the package root directory.
/// in that case, we will check if
/// `.gitignore` correctly contains `compile_commands.json` and
/// warn you if not.
///
/// hint: if `-fopenmp` is causing you trouble you can add
/// ``` yaml
/// CompileFlags:
///   Remove: [-fopenmp]
/// ```
/// to `user config root/clangd/config.yaml` or `project root/.clangd`.
/// user config root is determined by platform. run `clangd --help`
/// and look for the help of `--enable-config` to find more.
/// for example on macos, you should put above to
/// `~/Library/Preferences/clangd/config.yaml`.
pub fn make_compile_commands(builds: &[&Build]) {
    let project_root = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();
    let mut records = builds.into_iter().map(|build| {
        let ref_project_root = &project_root;
        let compiler = build.get_compiler();
        build.get_files().map(move |file| {
            let mut command = compiler.to_command();
            command.arg(file);
            CommandRecord {
                directory: ref_project_root.clone(),
                command: format!("{:?}", command),
                file: file.to_str().unwrap().to_string()
            }
        })
    }).flatten().collect::<HashSet<_>>();

    // import dependency commands
    for (k, v) in env::vars() {
        if !(k.starts_with("DEP_") && k.ends_with("_UCC_CSRC_COMPILE_COMMANDS")) {
            continue
        }
        println!("[ucc lsp] include dep commands {}", v);
        let file = File::open(v).unwrap();
        let reader = BufReader::new(file);
        let c: Vec<CommandRecord> = serde_json::from_reader(reader).unwrap();
        records.extend(c.into_iter());
    }

    let json = format!(
        "{}", serde_json::to_string_pretty(&records).unwrap());

    let save_to = PathBuf::from(out_dir).join("compile_commands.json");
    fs::write(&save_to, &json).unwrap();

    println!("cargo:ucc_csrc_compile_commands={}",
             save_to.display());

    if PathBuf::from(&project_root).join(".git").exists() {
        let save_to = PathBuf::from(&project_root).join("compile_commands.json");
        fs::write(save_to, &json).unwrap();

        // check .gitignore
        let gitignore = PathBuf::from(&project_root).join(".gitignore");
        if let Ok(gitignore) = fs::read_to_string(gitignore) {
            if !gitignore.contains("compile_commands.json") {
                println!("cargo:warning=.gitignore does not contain compile_commands.json");
                println!("cargo:rerun-if-changed=.gitignore");
            }
        }
        else {
            println!("cargo:warning=.gitignore file not found");
            println!("cargo:rerun-if-changed=.gitignore");
        }
    }
}
