#[inline(always)]
pub const fn manifest_dir() -> &'static str {
    env!("CARGO_MANIFEST_DIR")
}

#[cfg(test)]
mod tests {
    use super::manifest_dir;

    #[test]
    fn manifest_dir_points_to_admin_ui_client_crate() {
        assert!(manifest_dir().ends_with("graphile-worker-admin-ui-client"));
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm;
