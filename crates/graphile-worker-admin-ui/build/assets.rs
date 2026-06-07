use std::fs;
use std::path::Path;

pub(super) const PREBUILT_ASSETS: &[&str] =
    &["admin.css", "admin.js", "admin_ui.js", "admin_ui_bg.wasm"];

pub(super) fn prebuilt_assets_available(prebuilt_dir: &Path) -> bool {
    PREBUILT_ASSETS
        .iter()
        .all(|asset| prebuilt_dir.join(asset).is_file())
}

pub(super) fn copy_assets(from_dir: &Path, to_dir: &Path, description: &str) {
    fs::create_dir_all(to_dir).unwrap_or_else(|error| {
        panic!(
            "failed to create admin UI asset directory `{}`: {error}",
            to_dir.display()
        )
    });

    for asset in PREBUILT_ASSETS {
        fs::copy(from_dir.join(asset), to_dir.join(asset)).unwrap_or_else(|error| {
            panic!(
                "failed to {description} `{}`: {error}",
                from_dir.join(asset).display()
            )
        });
    }
}

pub(super) fn write_bootstrap(out_dir: &Path) {
    fs::write(
        out_dir.join("admin.js"),
        r#"import init from "./admin_ui.js";

const wasmUrl = new URL("./admin_ui_bg.wasm", import.meta.url);

init({ module_or_path: wasmUrl }).catch((error) => {
  console.error("Failed to initialize Graphile Worker Admin UI", error);
});
"#,
    )
    .expect("failed to write admin UI WASM bootstrap");
}
