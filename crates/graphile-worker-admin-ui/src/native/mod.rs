mod assets;
mod auth;
mod error;
mod middleware;
mod queries;
mod routes;
mod server;
mod state;
mod types;
mod view;

pub use auth::{generate_secret, AdminAuthConfig, AdminAuthSummary, PublicAuthMode};
pub use error::AdminUiError;
pub use server::{build_router, serve};
pub use state::{AdminServerConfig, AdminServerConfigBuilder};
pub use view::{render_admin_html, AdminUiRenderConfig};

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::sync::Arc;

    use axum::body::{to_bytes, Body};
    use axum::extract::State;
    use axum::http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
    use axum::http::{Request, StatusCode};
    use axum::Json;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use graphile_worker::WorkerUtils;
    use sqlx::{Postgres, QueryBuilder};

    use super::auth::{
        authorize_basic, generate_secret, AdminAuthConfig, AdminAuthSummary, PublicAuthMode,
    };
    use super::error::{AdminUiError, ApiError};
    use super::middleware::unauthorized_response;
    use super::queries::{apply_job_filters, job_lookup_error};
    use super::routes::add_job;
    use super::state::{AdminServerConfig, AppState};
    use super::types::{default_limit, AddJobRequest, JobKeyModeRequest, JobState, ListJobsParams};
    use super::view::{render_admin_html, AdminUiRenderConfig};

    #[test]
    fn generated_secret_is_hex_and_long_enough() {
        let secret = generate_secret();
        assert_eq!(secret.len(), 48);
        assert!(secret.chars().all(|c| c.is_ascii_hexdigit()));
    }

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

    #[test]
    fn basic_auth_accepts_correct_credentials() {
        let credentials = STANDARD.encode("admin:secret");
        let request = Request::builder()
            .header(AUTHORIZATION, format!("Basic {credentials}"))
            .body(Body::empty())
            .unwrap();

        assert!(authorize_basic(request.headers(), "admin", "secret"));
        assert!(!authorize_basic(request.headers(), "admin", "wrong"));
    }

    #[test]
    fn bearer_and_header_auth_accept_expected_tokens() {
        let bearer = Request::builder()
            .header(AUTHORIZATION, "Bearer admin-token")
            .body(Body::empty())
            .unwrap();
        assert!(AdminAuthConfig::bearer("admin-token", false).is_authorized(bearer.headers()));
        assert!(!AdminAuthConfig::bearer("other-token", false).is_authorized(bearer.headers()));

        let header = Request::builder()
            .header("x-admin-token", "header-token")
            .body(Body::empty())
            .unwrap();
        let auth = AdminAuthConfig::header("x-admin-token", "header-token", false).unwrap();
        assert!(auth.is_authorized(header.headers()));
    }

    #[tokio::test]
    async fn admin_server_config_builder_applies_safe_defaults() {
        let pool = lazy_pool();
        let database: graphile_worker::Database = pool.clone().into();
        let utils = WorkerUtils::new(database, "graphile_worker".to_string());

        let config = AdminServerConfig::builder(pool, utils).build().unwrap();

        assert_eq!(config.schema, "graphile_worker");
        assert_eq!(config.escaped_schema, "graphile_worker");
        assert_eq!(config.listen_addr, SocketAddr::from(([127, 0, 0, 1], 4000)));
        assert!(matches!(config.auth, AdminAuthConfig::Basic { .. }));
        assert!(!config.read_only);
    }

    #[tokio::test]
    async fn app_state_rejects_no_auth_on_unspecified_ipv4_addr() {
        let config = admin_config("0.0.0.0:4000".parse().unwrap(), AdminAuthConfig::None);

        let error = match AppState::from_config(config) {
            Ok(_) => panic!("no-auth admin UI must not bind to non-loopback addresses"),
            Err(error) => error,
        };

        assert!(matches!(error, AdminUiError::InsecureNoAuth));
    }

    #[tokio::test]
    async fn app_state_rejects_no_auth_on_unspecified_ipv6_addr() {
        let config = admin_config("[::]:4000".parse().unwrap(), AdminAuthConfig::None);

        let error = match AppState::from_config(config) {
            Ok(_) => panic!("no-auth admin UI must not bind to non-loopback addresses"),
            Err(error) => error,
        };

        assert!(matches!(error, AdminUiError::InsecureNoAuth));
    }

    #[tokio::test]
    async fn unauthorized_basic_response_prompts_for_basic_auth() {
        let response = unauthorized_response(&AdminAuthConfig::basic("admin", "secret"));
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers().contains_key(WWW_AUTHENTICATE));

        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(body.contains("unauthorized"));
    }

    #[tokio::test]
    async fn add_job_rejects_job_key_mode_without_key() {
        let pool = lazy_pool();
        let database: graphile_worker::Database = pool.clone().into();
        let state = Arc::new(AppState {
            pool,
            utils: WorkerUtils::new(database, "graphile_worker".to_string()),
            escaped_schema: "graphile_worker".to_string(),
            schema: "graphile_worker".to_string(),
            auth: AdminAuthConfig::None,
            csrf_token: "csrf".to_string(),
            read_only: false,
        });

        let error = add_job(
            State(state),
            Json(AddJobRequest {
                identifier: "send_email".to_string(),
                payload: serde_json::json!({}),
                queue: None,
                run_at: None,
                max_attempts: None,
                key: None,
                job_key_mode: Some(JobKeyModeRequest::Replace),
                priority: None,
                flags: None,
            }),
        )
        .await
        .expect_err("request should be rejected before reaching the database");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(error.message.contains("key"));
    }

    #[test]
    fn job_lookup_error_maps_missing_job_to_not_found() {
        let error = job_lookup_error(42, sqlx::Error::RowNotFound);

        assert_eq!(error.status, StatusCode::NOT_FOUND);
        assert!(error.message.contains("42"));
    }

    #[test]
    fn sqlx_row_not_found_maps_to_not_found() {
        let error = ApiError::from(sqlx::Error::RowNotFound);

        assert_eq!(error.status, StatusCode::NOT_FOUND);
        assert_eq!(error.message, "resource not found");
    }

    #[test]
    fn internal_errors_hide_error_details_from_clients() {
        let error = ApiError::internal("database password leaked");

        assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.message, "internal server error");
    }

    #[test]
    fn job_filters_ignore_whitespace_only_search() {
        let args = ListJobsParams {
            state: JobState::All,
            identifier: None,
            queue: None,
            search: Some("   ".to_string()),
            limit: default_limit(),
            offset: 0,
        };
        let mut query = QueryBuilder::<Postgres>::new("where true");

        apply_job_filters(&mut query, &args);

        assert_eq!(query.sql(), "where true");
    }

    fn lazy_pool() -> sqlx::PgPool {
        sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost/postgres")
            .unwrap()
    }

    fn admin_config(listen_addr: SocketAddr, auth: AdminAuthConfig) -> AdminServerConfig {
        let pool = lazy_pool();
        let database: graphile_worker::Database = pool.clone().into();
        AdminServerConfig {
            pool,
            utils: WorkerUtils::new(database, "graphile_worker".to_string()),
            escaped_schema: "graphile_worker".to_string(),
            schema: "graphile_worker".to_string(),
            listen_addr,
            auth,
            read_only: false,
        }
    }
}
