use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const ADMIN_UI_BUILD_HINT: &str = "Admin UI asset builds require npm, wasm-bindgen, and the wasm32-unknown-unknown Rust target. Install the Rust tooling with `rustup target add wasm32-unknown-unknown` and `cargo install wasm-bindgen-cli --version 0.2.121 --locked`.";
const REBUILD_ASSETS_ENV: &str = "GRAPHILE_WORKER_ADMIN_UI_REBUILD";
const UPDATE_PREBUILT_ASSETS_ENV: &str = "GRAPHILE_WORKER_ADMIN_UI_UPDATE_PREBUILT";
const PREBUILT_ASSETS: &[&str] = &["admin.css", "admin.js", "admin_ui.js", "admin_ui_bg.wasm"];
const CLIENT_MANIFEST_DIR: &str = graphile_worker_admin_ui_client::manifest_dir();

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let client_dir = PathBuf::from(CLIENT_MANIFEST_DIR);
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

fn rerun_if_changed(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.display());
}

fn rerun_if_env_changed(name: &str) {
    println!("cargo:rerun-if-env-changed={name}");
}

fn should_rebuild_assets(prebuilt_dir: &Path) -> bool {
    env_flag(REBUILD_ASSETS_ENV)
        || env_flag(UPDATE_PREBUILT_ASSETS_ENV)
        || !prebuilt_assets_available(prebuilt_dir)
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            matches!(
                value.as_str(),
                "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"
            )
        })
        .unwrap_or(false)
}

fn prebuilt_assets_available(prebuilt_dir: &Path) -> bool {
    PREBUILT_ASSETS
        .iter()
        .all(|asset| prebuilt_dir.join(asset).is_file())
}

fn copy_assets(from_dir: &Path, to_dir: &Path, description: &str) {
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

fn build_tailwind(manifest_dir: &Path, out_dir: &Path, client_dir: &Path) {
    let npm_dir = out_dir.join("npm");
    fs::create_dir_all(&npm_dir).expect("failed to create npm build directory");
    fs::copy(
        manifest_dir.join("package.json"),
        npm_dir.join("package.json"),
    )
    .expect("failed to copy package.json for admin UI asset build");
    fs::copy(
        manifest_dir.join("package-lock.json"),
        npm_dir.join("package-lock.json"),
    )
    .expect("failed to copy package-lock.json for admin UI asset build");

    run(
        Command::new("npm")
            .arg("ci")
            .arg("--ignore-scripts")
            .arg("--no-audit")
            .arg("--no-fund")
            .arg("--prefix")
            .arg(&npm_dir),
        "install admin UI npm dependencies",
    );

    let tailwind_config = out_dir.join("tailwind.config.cjs");
    fs::write(
        &tailwind_config,
        format!(
            r#"const {{ iconsPlugin, getIconCollections }} = require("@egoist/tailwindcss-icons");

module.exports = {{
  darkMode: "class",
  content: ["{}", "{}"],
  theme: {{
    extend: {{
      fontFamily: {{
        sans: [
          "Inter",
          "ui-sans-serif",
          "system-ui",
          "-apple-system",
          "BlinkMacSystemFont",
          "Segoe UI",
          "sans-serif",
        ],
      }},
    }},
  }},
  plugins: [
    iconsPlugin({{
      collections: getIconCollections(["lucide", "tabler"]),
      extraProperties: {{
        display: "inline-block",
        "vertical-align": "-0.16em",
      }},
    }}),
  ],
}};
"#,
            js_path(&manifest_dir.join("src/**/*.rs")),
            js_path(&client_dir.join("src/**/*.rs"))
        ),
    )
    .expect("failed to write generated Tailwind config for admin UI asset build");

    let tailwind_input = out_dir.join("tailwind.css");
    let css = fs::read_to_string(manifest_dir.join("assets/tailwind.css"))
        .expect("failed to read admin UI Tailwind input");
    let marker = "@config \"../tailwind.config.cjs\";";
    assert!(
        css.contains(marker),
        "admin UI tailwind.css is missing the expected `{marker}` directive",
    );
    let css = css.replace(
        marker,
        &format!("@config \"{}\";", js_path(&tailwind_config)),
    );
    fs::write(&tailwind_input, css)
        .expect("failed to write generated Tailwind input for admin UI asset build");

    let tailwind = npm_bin(&npm_dir, "tailwindcss");
    run(
        Command::new(tailwind)
            .arg("-i")
            .arg(&tailwind_input)
            .arg("-o")
            .arg(out_dir.join("admin.css"))
            .arg("--minify")
            .current_dir(manifest_dir)
            .env("NODE_PATH", npm_dir.join("node_modules")),
        "build admin UI Tailwind CSS",
    );
}

fn build_wasm_client(client_dir: &Path, out_dir: &Path) {
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

fn js_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace('"', "\\\"")
}

fn clear_clippy_env(command: &mut Command) {
    for key in [
        "CLIPPY_ARGS",
        "CLIPPY_CONF_DIR",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
    ] {
        command.env_remove(key);
    }
}

fn write_bootstrap(out_dir: &Path) {
    fs::write(
        out_dir.join("admin.js"),
        r#"import init from "/assets/admin_ui.js";

const wasmUrl = new URL("/assets/admin_ui_bg.wasm", `${location.protocol}//${location.host}`);

init({ module_or_path: wasmUrl }).catch((error) => {
  console.error("Failed to initialize Graphile Worker Admin UI", error);
});
"#,
    )
    .expect("failed to write admin UI WASM bootstrap");
}

fn npm_bin(npm_dir: &Path, bin: &str) -> PathBuf {
    let bin_name = if cfg!(windows) {
        format!("{bin}.cmd")
    } else {
        bin.to_string()
    };
    npm_dir.join("node_modules").join(".bin").join(bin_name)
}

fn run(command: &mut Command, description: &str) {
    run_with_hint(command, description, None);
}

fn run_with_hint(command: &mut Command, description: &str, hint: Option<&str>) {
    let program = command.get_program().to_owned();
    let args = command
        .get_args()
        .map(OsStr::to_string_lossy)
        .collect::<Vec<_>>()
        .join(" ");
    let hint = hint.map(|hint| format!("\n{hint}")).unwrap_or_default();

    let status = command.status().unwrap_or_else(|error| {
        panic!(
            "failed to {description}: could not run `{}`: {error}{hint}",
            program.to_string_lossy()
        )
    });

    if !status.success() {
        panic!(
            "failed to {description}: `{}` exited with status {status}{hint}",
            format!("{} {args}", program.to_string_lossy()).trim()
        );
    }
}
