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

    // --- 阶段 2：固化可跨平台传输的 Linux 运行包 ---
    let release_dir = manifest_dir.join("target/release");
    let core_runtime = env::var_os("HARBOR_CORE_RUNTIME_BUNDLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| release_dir.join("harbor_core-runtime-linux-x86_64.tar.gz"));
    export_runtime_bundle(&core_runtime, "harbor_core", "HARBOR_CORE");

    let ttyd_runtime = env::var_os("HARBOR_TTYD_RUNTIME_BUNDLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| release_dir.join("harbor_ttyd-runtime-linux-x86_64.tar.gz"));
    export_runtime_bundle(&ttyd_runtime, "harbor_ttyd", "HARBOR_TTYD");

    // --- 阶段 3：Linux GUI 额外固化本机 Core 二进制 ---
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        let core_path = env::var_os("HARBOR_CORE_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| release_dir.join("harbor_core"));
        let (core_hash, hash_path, cached_core_path) = verified_artifact(&core_path, "harbor_core");
        let runtime = runtime_metadata(&core_runtime);
        if runtime.binary_sha256 != core_hash {
            panic!(
                "harbor_core runtime contains binary {}, but local core is {core_hash}",
                runtime.binary_sha256
            );
        }
        println!(
            "cargo:rustc-env=HARBOR_CORE_BUILD_PATH={}",
            cached_core_path.display()
        );
        println!("cargo:rerun-if-changed={}", core_path.display());
        println!("cargo:rerun-if-changed={}", hash_path.display());
        println!("cargo:rerun-if-changed={}", cached_core_path.display());
    }

    // --- 阶段 4：生成 Tauri 构建信息 ---
    tauri_build::build()
}

struct RuntimeMetadata {
    architecture: String,
    binary_sha256: String,
    archive_sha256: String,
    loader: String,
}

fn export_runtime_bundle(path: &Path, name: &str, prefix: &str) {
    if !path.is_file() {
        panic!(
            "{name} runtime bundle not found at {}; run npm run core:release before building Harbor",
            path.display()
        );
    }
    let metadata = runtime_metadata(path);
    if metadata.architecture != "x86_64" {
        panic!(
            "unsupported {name} runtime architecture {}",
            metadata.architecture
        );
    }
    let actual_hash = sha256_file(path);
    if metadata.archive_sha256 != actual_hash {
        panic!(
            "{name} runtime hash is stale: manifest {}, actual {actual_hash}",
            metadata.archive_sha256
        );
    }
    let hash_path = PathBuf::from(format!("{}.sha256", path.display()));
    let recorded_hash = std::fs::read_to_string(&hash_path)
        .unwrap_or_else(|error| panic!("read {} failed: {error}", hash_path.display()));
    if recorded_hash.trim() != actual_hash {
        panic!(
            "{name} runtime SHA-256 file is stale: manifest {}, actual {actual_hash}",
            recorded_hash.trim()
        );
    }
    println!("cargo:rustc-env={prefix}_SHA256={}", metadata.binary_sha256);
    println!("cargo:rustc-env={prefix}_RUNTIME_SHA256={actual_hash}");
    println!(
        "cargo:rustc-env={prefix}_RUNTIME_LOADER={}",
        metadata.loader
    );
    println!(
        "cargo:rustc-env={prefix}_RUNTIME_BUILD_PATH={}",
        path.display()
    );
    println!("cargo:rerun-if-changed={}", path.display());
    println!("cargo:rerun-if-changed={}", hash_path.display());
    println!("cargo:rerun-if-changed={}.env", path.display());
}

fn runtime_metadata(path: &Path) -> RuntimeMetadata {
    let env_path = PathBuf::from(format!("{}.env", path.display()));
    let raw = std::fs::read_to_string(&env_path)
        .unwrap_or_else(|error| panic!("read {} failed: {error}", env_path.display()));
    let value = |key: &str| {
        raw.lines()
            .find_map(|line| line.split_once('=').filter(|(name, _)| *name == key))
            .map(|(_, value)| value.to_string())
            .unwrap_or_else(|| panic!("{} is missing {key}", env_path.display()))
    };
    RuntimeMetadata {
        architecture: value("architecture"),
        binary_sha256: value("binary_sha256"),
        archive_sha256: value("archive_sha256"),
        loader: value("loader"),
    }
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
