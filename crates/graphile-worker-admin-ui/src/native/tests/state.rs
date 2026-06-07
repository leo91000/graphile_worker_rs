use super::*;

#[tokio::test]
async fn admin_server_config_builder_applies_safe_defaults() {
    let pool = lazy_pool();
    let database: graphile_worker::Database = pool.clone().into();
    let utils = WorkerUtils::new(database, "graphile_worker".to_string());

    let config = AdminServerConfig::builder(pool, utils).build().unwrap();

    assert_eq!(config.schema, "graphile_worker");
    assert_eq!(config.schema_name, "graphile_worker");
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
