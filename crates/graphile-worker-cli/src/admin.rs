use anyhow::{anyhow, Context, Result};
use graphile_worker_admin_ui::AdminAuthConfig;

use crate::args::{AdminArgs, AdminAuthModeArg};

pub(crate) fn build_admin_auth(args: &AdminArgs) -> Result<AdminAuthConfig> {
    match args.auth {
        AdminAuthModeArg::Basic => Ok(match &args.password {
            Some(password) => AdminAuthConfig::basic(&args.username, password),
            None => AdminAuthConfig::basic_with_random_password(&args.username),
        }),
        AdminAuthModeArg::Bearer => {
            let (token, generated) = match &args.bearer_token {
                Some(token) => (token.clone(), false),
                None => (graphile_worker_admin_ui::generate_secret(), true),
            };
            Ok(AdminAuthConfig::bearer(token, generated))
        }
        AdminAuthModeArg::Header => {
            let (token, generated) = match &args.header_token {
                Some(token) => (token.clone(), false),
                None => (graphile_worker_admin_ui::generate_secret(), true),
            };
            AdminAuthConfig::header(&args.header_name, token, generated)
                .context("invalid admin header auth configuration")
        }
        AdminAuthModeArg::None => {
            if !args.listen.ip().is_loopback() {
                return Err(anyhow!(
                    "--auth none is only allowed when --listen uses a loopback address"
                ));
            }
            Ok(AdminAuthConfig::None)
        }
    }
}

pub(crate) fn print_admin_startup(args: &AdminArgs, auth: &AdminAuthConfig) {
    println!("Graphile Worker admin UI: http://{}", args.listen);
    println!(
        "Read-only mode: {}",
        if args.read_only { "on" } else { "off" }
    );

    match auth {
        AdminAuthConfig::Basic {
            username,
            generated_password,
            ..
        } => {
            println!("Auth: HTTP Basic");
            println!("Username: {username}");
            if *generated_password {
                if let Some(password) = auth.secret_for_display() {
                    println!("Generated password: {password}");
                }
            } else {
                println!("Password: configured");
            }
        }
        AdminAuthConfig::Bearer {
            generated_token, ..
        } => {
            println!("Auth: bearer token");
            if *generated_token {
                if let Some(token) = auth.secret_for_display() {
                    println!("Generated bearer token: {token}");
                }
            } else {
                println!("Bearer token: configured");
            }
        }
        AdminAuthConfig::Header {
            header_name,
            generated_token,
            ..
        } => {
            println!("Auth: header token");
            println!("Header: {}", header_name.as_str());
            if *generated_token {
                if let Some(token) = auth.secret_for_display() {
                    println!("Generated header token: {token}");
                }
            } else {
                println!("Header token: configured");
            }
        }
        AdminAuthConfig::None => {
            println!("Auth: none (loopback only)");
        }
    }
}
