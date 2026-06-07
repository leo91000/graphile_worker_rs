use std::env;
use std::path::{Path, PathBuf};

#[path = "build/assets.rs"]
mod assets;
#[path = "build/commands.rs"]
mod commands;
#[path = "build/paths.rs"]
mod paths;
#[path = "build/tailwind.rs"]
mod tailwind;
#[path = "build/wasm.rs"]
mod wasm;

use assets::{copy_assets, prebuilt_assets_available, write_bootstrap, PREBUILT_ASSETS};
use paths::{env_flag, rerun_if_changed, rerun_if_env_changed};
use tailwind::build_tailwind;
use wasm::build_wasm_client;

const REBUILD_ASSETS_ENV: &str = "GRAPHILE_WORKER_ADMIN_UI_REBUILD";
const UPDATE_PREBUILT_ASSETS_ENV: &str = "GRAPHILE_WORKER_ADMIN_UI_UPDATE_PREBUILT";

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let client_dir = manifest_dir
        .parent()
        .map(|crates_dir| crates_dir.join("graphile-worker-admin-ui-client"))
        .unwrap_or_else(|| manifest_dir.join("../graphile-worker-admin-ui-client"));
    let prebuilt_dir = manifest_dir.join("prebuilt");

    rerun_if_env_changed(REBUILD_ASSETS_ENV);
    rerun_if_env_changed(UPDATE_PREBUILT_ASSETS_ENV);
    for asset in PREBUILT_ASSETS {
        rerun_if_changed(&prebuilt_dir.join(asset));
    }
    rerun_if_changed(&manifest_dir.join("assets/tailwind.css"));
    rerun_if_changed(&manifest_dir.join("tailwind.config.cjs"));
    rerun_if_changed(&manifest_dir.join("package.json"));
    rerun_if_changed(&manifest_dir.join("package-lock.json"));
    rerun_if_changed(&client_dir.join("Cargo.toml"));
    rerun_if_changed(&client_dir.join("src/lib.rs"));

    if should_rebuild_assets(&prebuilt_dir) {
        build_tailwind(&manifest_dir, &out_dir, &client_dir);
        build_wasm_client(&client_dir, &out_dir);
        write_bootstrap(&out_dir);

        if env_flag(UPDATE_PREBUILT_ASSETS_ENV) {
            copy_assets(&out_dir, &prebuilt_dir, "update prebuilt admin UI asset");
        }
        return;
    }

    copy_assets(&prebuilt_dir, &out_dir, "copy prebuilt admin UI asset");
}

fn should_rebuild_assets(prebuilt_dir: &Path) -> bool {
    env_flag(REBUILD_ASSETS_ENV)
        || env_flag(UPDATE_PREBUILT_ASSETS_ENV)
        || !prebuilt_assets_available(prebuilt_dir)
}
