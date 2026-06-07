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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(auth: AdminAuthModeArg) -> AdminArgs {
        AdminArgs {
            listen: "127.0.0.1:5678".parse().unwrap(),
            auth,
            username: "admin".to_string(),
            password: None,
            bearer_token: None,
            header_token: None,
            header_name: "x-test-token".to_string(),
            read_only: false,
        }
    }

    #[test]
    fn builds_admin_auth_modes() {
        let mut basic = args(AdminAuthModeArg::Basic);
        basic.password = Some("secret".to_string());
        assert!(matches!(
            build_admin_auth(&basic).unwrap(),
            AdminAuthConfig::Basic {
                generated_password: false,
                ..
            }
        ));

        assert!(matches!(
            build_admin_auth(&args(AdminAuthModeArg::Basic)).unwrap(),
            AdminAuthConfig::Basic {
                generated_password: true,
                ..
            }
        ));

        let mut bearer = args(AdminAuthModeArg::Bearer);
        bearer.bearer_token = Some("bearer-token".to_string());
        assert!(matches!(
            build_admin_auth(&bearer).unwrap(),
            AdminAuthConfig::Bearer {
                generated_token: false,
                ..
            }
        ));
        assert!(matches!(
            build_admin_auth(&args(AdminAuthModeArg::Bearer)).unwrap(),
            AdminAuthConfig::Bearer {
                generated_token: true,
                ..
            }
        ));

        let mut header = args(AdminAuthModeArg::Header);
        header.header_token = Some("header-token".to_string());
        assert!(matches!(
            build_admin_auth(&header).unwrap(),
            AdminAuthConfig::Header {
                generated_token: false,
                ..
            }
        ));
        assert!(matches!(
            build_admin_auth(&args(AdminAuthModeArg::Header)).unwrap(),
            AdminAuthConfig::Header {
                generated_token: true,
                ..
            }
        ));

        assert!(matches!(
            build_admin_auth(&args(AdminAuthModeArg::None)).unwrap(),
            AdminAuthConfig::None
        ));
    }

    #[test]
    fn rejects_unsafe_or_invalid_admin_auth_config() {
        let mut non_loopback = args(AdminAuthModeArg::None);
        non_loopback.listen = "0.0.0.0:5678".parse().unwrap();
        assert!(build_admin_auth(&non_loopback)
            .unwrap_err()
            .to_string()
            .contains("loopback"));

        let mut invalid_header = args(AdminAuthModeArg::Header);
        invalid_header.header_name = "not a header".to_string();
        assert!(build_admin_auth(&invalid_header).is_err());
    }

    #[test]
    fn prints_admin_startup_for_each_auth_mode() {
        let mut basic = args(AdminAuthModeArg::Basic);
        basic.password = Some("secret".to_string());
        let basic_auth = build_admin_auth(&basic).unwrap();
        print_admin_startup(&basic, &basic_auth);

        let generated_basic = args(AdminAuthModeArg::Basic);
        let generated_basic_auth = build_admin_auth(&generated_basic).unwrap();
        print_admin_startup(&generated_basic, &generated_basic_auth);

        let mut bearer = args(AdminAuthModeArg::Bearer);
        bearer.bearer_token = Some("bearer-token".to_string());
        let bearer_auth = build_admin_auth(&bearer).unwrap();
        print_admin_startup(&bearer, &bearer_auth);

        let generated_bearer = args(AdminAuthModeArg::Bearer);
        let generated_bearer_auth = build_admin_auth(&generated_bearer).unwrap();
        print_admin_startup(&generated_bearer, &generated_bearer_auth);

        let mut header = args(AdminAuthModeArg::Header);
        header.header_token = Some("header-token".to_string());
        let header_auth = build_admin_auth(&header).unwrap();
        print_admin_startup(&header, &header_auth);

        let generated_header = args(AdminAuthModeArg::Header);
        let generated_header_auth = build_admin_auth(&generated_header).unwrap();
        print_admin_startup(&generated_header, &generated_header_auth);

        let none = args(AdminAuthModeArg::None);
        let none_auth = build_admin_auth(&none).unwrap();
        print_admin_startup(&none, &none_auth);
    }
}
