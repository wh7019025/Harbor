use std::env;
use std::path::Path;
use std::process::Command;

pub fn export(root: &Path) {
    let script = root.join("scripts/git_version.sh");
    let version = env::var("HARBOR_VERSION").unwrap_or_else(|_| {
        let output = Command::new(&script)
            .output()
            .unwrap_or_else(|error| panic!("run {} failed: {error}", script.display()));
        if !output.status.success() {
            panic!(
                "resolve Harbor Git version failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        String::from_utf8(output.stdout)
            .unwrap_or_else(|error| panic!("Harbor Git version is not UTF-8: {error}"))
            .trim()
            .to_string()
    });
    if version.is_empty() || version.contains('\r') || version.contains('\n') {
        panic!("invalid Harbor Git version: {version:?}");
    }

    println!("cargo:rustc-env=HARBOR_VERSION={version}");
    println!("cargo:rerun-if-env-changed=HARBOR_VERSION");
    println!("cargo:rerun-if-changed={}", script.display());
    emit_git_rerun_paths(root);
}

fn emit_git_rerun_paths(root: &Path) {
    let git_dir = command_text(root, &["rev-parse", "--absolute-git-dir"]);
    let Some(git_dir) = git_dir else {
        return;
    };
    let git_dir = Path::new(&git_dir);
    println!("cargo:rerun-if-changed={}", git_dir.join("HEAD").display());
    println!(
        "cargo:rerun-if-changed={}",
        git_dir.join("packed-refs").display()
    );
    if let Some(reference) = command_text(root, &["symbolic-ref", "-q", "HEAD"]) {
        println!(
            "cargo:rerun-if-changed={}",
            git_dir.join(reference).display()
        );
    }
}

fn command_text(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}
