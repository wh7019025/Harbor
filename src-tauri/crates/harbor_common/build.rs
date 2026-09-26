use std::path::PathBuf;

#[path = "../../build_support/git_version.rs"]
mod git_version;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    git_version::export(&manifest_dir.join("../../.."));
}
