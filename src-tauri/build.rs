use std::env;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

#[path = "build_support/git_version.rs"]
mod git_version;

fn main() {
    // --- 阶段 1：从 Git 注入 GUI 版本 ---
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    git_version::export(&manifest_dir.join(".."));

    // --- 阶段 2：定位已经完成构建的 release core ---
    let core_path = env::var_os("HARBOR_CORE_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("target/release/harbor_core"));
    if !core_path.is_file() {
        panic!(
            "release harbor_core not found at {}; build it before harbor",
            core_path.display()
        );
    }
    // --- 阶段 3：计算哈希并固化到 GUI 二进制 ---
    let (core_hash, hash_path, cached_core_path) = verified_artifact(&core_path, "harbor_core");
    println!("cargo:rustc-env=HARBOR_CORE_SHA256={core_hash}");
    println!(
        "cargo:rustc-env=HARBOR_CORE_BUILD_PATH={}",
        cached_core_path.display()
    );
    println!("cargo:rerun-if-changed={}", core_path.display());
    println!("cargo:rerun-if-changed={}", hash_path.display());
    println!("cargo:rerun-if-changed={}", cached_core_path.display());

    // --- 阶段 4：固化 Harbor 托管 ttyd 的路径与哈希 ---
    let ttyd_path = core_path.with_file_name("harbor_ttyd");
    if !ttyd_path.is_file() {
        panic!(
            "managed ttyd not found at {}; run npm run core:release before building harbor",
            ttyd_path.display()
        );
    }
    let (ttyd_hash, ttyd_hash_path, cached_ttyd_path) =
        verified_artifact(&ttyd_path, "harbor_ttyd");
    println!("cargo:rustc-env=HARBOR_TTYD_SHA256={ttyd_hash}");
    println!(
        "cargo:rustc-env=HARBOR_TTYD_BUILD_PATH={}",
        cached_ttyd_path.display()
    );
    println!("cargo:rerun-if-changed={}", ttyd_path.display());
    println!("cargo:rerun-if-changed={}", ttyd_hash_path.display());
    println!("cargo:rerun-if-changed={}", cached_ttyd_path.display());

    // --- 阶段 5：生成 Tauri 构建信息 ---
    tauri_build::build()
}

fn verified_artifact(path: &Path, name: &str) -> (String, PathBuf, PathBuf) {
    let hash_path = path.with_file_name(format!("{name}.sha256"));
    if !hash_path.is_file() {
        panic!(
            "{name} hash not found at {}; run npm run core:release before building harbor",
            hash_path.display()
        );
    }
    let hash = sha256_file(path);
    let recorded_hash = std::fs::read_to_string(&hash_path)
        .unwrap_or_else(|error| panic!("read {} failed: {error}", hash_path.display()));
    let recorded_hash = recorded_hash.trim();
    if recorded_hash != hash {
        panic!("{name} hash is stale: manifest {recorded_hash}, actual {hash}");
    }
    let cache_dir_name = format!("{name}-builds");
    let cached_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(cache_dir_name)
        .join(hash.as_str());
    if !cached_path.is_file() {
        panic!(
            "immutable {name} not found at {}; run npm run core:release before building harbor",
            cached_path.display()
        );
    }
    let cached_hash = sha256_file(&cached_path);
    if cached_hash != hash {
        panic!("immutable {name} hash mismatch: expected {hash}, got {cached_hash}");
    }
    (hash, hash_path, cached_path)
}

fn sha256_file(path: &Path) -> String {
    let mut file = File::open(path)
        .unwrap_or_else(|error| panic!("open release core {} failed: {error}", path.display()));
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .unwrap_or_else(|error| panic!("read release core {} failed: {error}", path.display()));
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    format!("{:x}", hasher.finalize())
}
