use std::ffi::OsStr;
use std::process::Command;

pub(super) fn clear_clippy_env(command: &mut Command) {
    for key in [
        "CLIPPY_ARGS",
        "CLIPPY_CONF_DIR",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
    ] {
        command.env_remove(key);
    }
}

pub(super) fn run(command: &mut Command, description: &str) {
    run_with_hint(command, description, None);
}

pub(super) fn run_with_hint(command: &mut Command, description: &str, hint: Option<&str>) {
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
