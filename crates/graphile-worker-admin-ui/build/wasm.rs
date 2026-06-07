use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use super::commands::{clear_clippy_env, run_with_hint};

const ADMIN_UI_BUILD_HINT: &str = "Admin UI asset builds require npm, wasm-bindgen, and the wasm32-unknown-unknown Rust target. Install the Rust tooling with `rustup target add wasm32-unknown-unknown` and `cargo install wasm-bindgen-cli --version 0.2.121 --locked`.";

pub(super) fn build_wasm_client(client_dir: &Path, out_dir: &Path) {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let target_dir = out_dir.join("wasm-target");
    let mut cargo = Command::new(cargo);
    cargo
        .arg("build")
        .arg("--manifest-path")
        .arg(client_dir.join("Cargo.toml"))
        .arg("--lib")
        .arg("--target")
        .arg("wasm32-unknown-unknown")
        .arg("--release")
        .arg("--target-dir")
        .arg(&target_dir);
    clear_clippy_env(&mut cargo);

    run_with_hint(
        &mut cargo,
        "build admin UI Leptos WASM client",
        Some(ADMIN_UI_BUILD_HINT),
    );

    let wasm = target_dir
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("graphile_worker_admin_ui_client.wasm");
    run_with_hint(
        Command::new("wasm-bindgen")
            .arg(&wasm)
            .arg("--target")
            .arg("web")
            .arg("--out-dir")
            .arg(out_dir)
            .arg("--out-name")
            .arg("admin_ui"),
        "run wasm-bindgen for admin UI client",
        Some(ADMIN_UI_BUILD_HINT),
    );

    let _ = fs::remove_file(out_dir.join("admin_ui.d.ts"));
    let _ = fs::remove_file(out_dir.join("admin_ui_bg.wasm.d.ts"));
}
