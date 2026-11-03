//! Rebuilds wpg-server when the web UI output (`crates/wpg-web/dist`, written by
//! `trunk build`) changes, so a release binary never keeps an outdated or missing UI.
//!
//! `rust-embed` reads the folder at compile time but does not tell Cargo about it.
//! `rerun-if-changed` on a directory makes Cargo scan it recursively for modified or
//! new files. A missing path would make Cargo rerun this script on every build, so the
//! (git-ignored) directory is created empty first; the server then serves its
//! placeholder page until the UI is built.

use std::path::Path;

fn main() {
    let dist = Path::new("../wpg-web/dist");
    println!("cargo:rerun-if-changed=build.rs");
    if dist.is_dir() || std::fs::create_dir_all(dist).is_ok() {
        println!("cargo:rerun-if-changed=../wpg-web/dist");
    } else {
        // Read-only source tree: watch the parent, which changes when dist/ appears.
        println!("cargo:rerun-if-changed=../wpg-web");
    }
}
