use std::fs;
use std::path::Path;
use std::process::Command;

use super::commands::run;
use super::paths::{js_path, npm_bin};

pub(super) fn build_tailwind(manifest_dir: &Path, out_dir: &Path, client_dir: &Path) {
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
