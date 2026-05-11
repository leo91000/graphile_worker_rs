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

pub struct AdminServerConfigBuilder {
    pool: PgPool,
    utils: WorkerUtils,
    escaped_schema: String,
    schema: String,
    listen_addr: SocketAddr,
    auth: AdminAuthConfig,
    read_only: bool,
}

impl AdminServerConfig {
    pub fn builder(pool: PgPool, utils: WorkerUtils) -> AdminServerConfigBuilder {
        AdminServerConfigBuilder {
            pool,
            utils,
            escaped_schema: "graphile_worker".to_string(),
            schema: "graphile_worker".to_string(),
            listen_addr: SocketAddr::from(([127, 0, 0, 1], 4000)),
            auth: AdminAuthConfig::basic_with_random_password("admin"),
            read_only: false,
        }
    }

    pub fn validate(&self) -> Result<(), AdminUiError> {
        if matches!(self.auth, AdminAuthConfig::None) && !self.listen_addr.ip().is_loopback() {
            return Err(AdminUiError::InsecureNoAuth);
        }
        Ok(())
    }
}

impl AdminServerConfigBuilder {
    pub fn escaped_schema(mut self, escaped_schema: impl Into<String>) -> Self {
        self.escaped_schema = escaped_schema.into();
        self
    }

    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = schema.into();
        self
    }

    pub fn listen_addr(mut self, listen_addr: SocketAddr) -> Self {
        self.listen_addr = listen_addr;
        self
    }

    pub fn auth(mut self, auth: AdminAuthConfig) -> Self {
        self.auth = auth;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn build(self) -> Result<AdminServerConfig, AdminUiError> {
        let config = AdminServerConfig {
            pool: self.pool,
            utils: self.utils,
            escaped_schema: self.escaped_schema,
            schema: self.schema,
            listen_addr: self.listen_addr,
            auth: self.auth,
            read_only: self.read_only,
        };
        config.validate()?;
        Ok(config)
    }
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
        config.validate()?;

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
