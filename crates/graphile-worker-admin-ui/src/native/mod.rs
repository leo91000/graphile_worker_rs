mod assets;
mod auth;
mod error;
mod middleware;
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
mod tests;
