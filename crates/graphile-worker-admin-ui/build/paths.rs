use std::env;
use std::path::{Path, PathBuf};

pub(super) fn rerun_if_changed(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.display());
}

pub(super) fn rerun_if_env_changed(name: &str) {
    println!("cargo:rerun-if-env-changed={name}");
}

pub(super) fn env_flag(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            matches!(
                value.as_str(),
                "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"
            )
        })
        .unwrap_or(false)
}

pub(super) fn js_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace('"', "\\\"")
}

pub(super) fn npm_bin(npm_dir: &Path, bin: &str) -> PathBuf {
    let bin_name = if cfg!(windows) {
        format!("{bin}.cmd")
    } else {
        bin.to_string()
    };
    npm_dir.join("node_modules").join(".bin").join(bin_name)
}
