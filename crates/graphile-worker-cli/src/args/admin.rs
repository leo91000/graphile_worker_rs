use std::net::SocketAddr;

use clap::{Args, ValueEnum};

#[derive(Args, Debug)]
pub(crate) struct AdminArgs {
    /// Address for the admin HTTP server.
    #[arg(long, default_value = "127.0.0.1:5678")]
    pub(crate) listen: SocketAddr,

    /// Authentication mode for the admin UI.
    #[arg(long, value_enum, default_value_t = AdminAuthModeArg::Basic)]
    pub(crate) auth: AdminAuthModeArg,

    /// HTTP Basic username.
    #[arg(long, default_value = "admin")]
    pub(crate) username: String,

    /// HTTP Basic password. Generated randomly when omitted.
    #[arg(long, env = "GRAPHILE_WORKER_ADMIN_PASSWORD")]
    pub(crate) password: Option<String>,

    /// Bearer token. Generated randomly when --auth bearer and omitted.
    #[arg(long, env = "GRAPHILE_WORKER_ADMIN_BEARER_TOKEN")]
    pub(crate) bearer_token: Option<String>,

    /// Header token. Generated randomly when --auth header and omitted.
    #[arg(long, env = "GRAPHILE_WORKER_ADMIN_HEADER_TOKEN")]
    pub(crate) header_token: Option<String>,

    /// Header name for --auth header.
    #[arg(long, default_value = "x-graphile-worker-admin-token")]
    pub(crate) header_name: String,

    /// Disable all mutating admin actions.
    #[arg(long)]
    pub(crate) read_only: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum AdminAuthModeArg {
    Basic,
    Bearer,
    Header,
    None,
}
