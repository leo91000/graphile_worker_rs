use std::net::SocketAddr;

use graphile_worker::WorkerUtils;
use sqlx::PgPool;

use super::auth::{generate_secret, AdminAuthConfig};
use super::error::AdminUiError;

#[derive(Clone)]
pub struct AdminServerConfig {
    pub pool: PgPool,
    pub utils: WorkerUtils,
    pub escaped_schema: String,
    pub schema: String,
    pub listen_addr: SocketAddr,
    pub auth: AdminAuthConfig,
    pub read_only: bool,
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) pool: PgPool,
    pub(crate) utils: WorkerUtils,
    pub(crate) escaped_schema: String,
    pub(crate) schema: String,
    pub(crate) auth: AdminAuthConfig,
    pub(crate) csrf_token: String,
    pub(crate) read_only: bool,
}

impl AppState {
    pub(crate) fn from_config(config: AdminServerConfig) -> Result<Self, AdminUiError> {
        if matches!(config.auth, AdminAuthConfig::None) && !config.listen_addr.ip().is_loopback() {
            return Err(AdminUiError::InsecureNoAuth);
        }

        Ok(Self {
            pool: config.pool,
            utils: config.utils,
            escaped_schema: config.escaped_schema,
            schema: config.schema,
            auth: config.auth,
            csrf_token: generate_secret(),
            read_only: config.read_only,
        })
    }
}
