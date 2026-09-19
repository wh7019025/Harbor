use std::env;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

fn main() {
    // --- 阶段 1：定位已经完成构建的 release core ---
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let core_path = env::var_os("HARBOR_CORE_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("target/release/harbor_core"));
    if !core_path.is_file() {
        panic!(
            "release harbor_core not found at {}; build it before harbor",
            core_path.display()
        );
    }
    let hash_path = core_path.with_file_name("harbor_core.sha256");
    if !hash_path.is_file() {
        panic!(
            "release harbor_core hash not found at {}; run npm run core:release before building harbor",
            hash_path.display()
        );
    }

    // --- 阶段 2：计算哈希并固化到 GUI 二进制 ---
    let core_hash = sha256_file(&core_path);
    let recorded_hash = std::fs::read_to_string(&hash_path)
        .unwrap_or_else(|error| panic!("read {} failed: {error}", hash_path.display()));
    let recorded_hash = recorded_hash.trim();
    if recorded_hash != core_hash {
        panic!("release harbor_core hash is stale: manifest {recorded_hash}, actual {core_hash}");
    }
    let cached_core_path = core_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("harbor_core-builds")
        .join(core_hash.as_str());
    if !cached_core_path.is_file() {
        panic!(
            "immutable release harbor_core not found at {}; run npm run core:release before building harbor",
            cached_core_path.display()
        );
    }
    let cached_hash = sha256_file(&cached_core_path);
    if cached_hash != core_hash {
        panic!(
            "immutable release harbor_core hash mismatch: expected {core_hash}, got {cached_hash}"
        );
    }
    println!("cargo:rustc-env=HARBOR_CORE_SHA256={core_hash}");
    println!(
        "cargo:rustc-env=HARBOR_CORE_BUILD_PATH={}",
        cached_core_path.display()
    );
    println!("cargo:rerun-if-changed={}", core_path.display());
    println!("cargo:rerun-if-changed={}", hash_path.display());
    println!("cargo:rerun-if-changed={}", cached_core_path.display());

    // --- 阶段 3：生成 Tauri 构建信息 ---
    tauri_build::build()
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
