use std::net::SocketAddr;

use graphile_worker::{Schema, WorkerUtils};
use sqlx::PgPool;

use super::auth::{generate_secret, AdminAuthConfig};
use super::error::AdminUiError;

#[derive(Clone)]
pub struct AdminServerConfig {
    pub pool: PgPool,
    pub utils: WorkerUtils,
    pub schema: Schema,
    pub schema_name: String,
    pub listen_addr: SocketAddr,
    pub auth: AdminAuthConfig,
    pub read_only: bool,
}

pub struct AdminServerConfigBuilder {
    pool: PgPool,
    utils: WorkerUtils,
    schema: Schema,
    schema_name: String,
    listen_addr: SocketAddr,
    auth: AdminAuthConfig,
    read_only: bool,
}

impl AdminServerConfig {
    pub fn builder(pool: PgPool, utils: WorkerUtils) -> AdminServerConfigBuilder {
        AdminServerConfigBuilder {
            pool,
            utils,
            schema: Schema::default(),
            schema_name: "graphile_worker".to_string(),
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
    pub fn schema(mut self, schema: impl Into<Schema>) -> Self {
        self.schema = schema.into();
        self
    }

    pub fn schema_name(mut self, schema_name: impl Into<String>) -> Self {
        self.schema_name = schema_name.into();
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
            schema: self.schema,
            schema_name: self.schema_name,
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
    pub(crate) schema: Schema,
    pub(crate) schema_name: String,
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
            schema: config.schema,
            schema_name: config.schema_name,
            auth: config.auth,
            csrf_token: generate_secret(),
            read_only: config.read_only,
        })
    }
}
