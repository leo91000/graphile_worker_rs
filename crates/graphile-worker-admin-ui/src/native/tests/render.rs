use super::*;

#[test]
fn render_includes_embedded_bootstrap_data_and_icons() {
    let html = render_admin_html(&AdminUiRenderConfig {
        csrf_token: "csrf".to_string(),
        schema: "graphile_worker".to_string(),
        read_only: false,
        auth: AdminAuthSummary {
            mode: PublicAuthMode::Basic,
            username: Some("admin".to_string()),
            header_name: None,
            generated_secret: true,
        },
    });

    assert!(html.contains("data-auth-mode=\"basic\""));
    assert!(html.contains("data-csrf=\"csrf\""));
    assert!(html.contains("i-lucide-refresh-cw"));
    assert!(html.contains("i-tabler-tool"));
    assert!(html.contains("/assets/admin.css"));
    assert!(html.contains("/assets/admin.js"));
}
