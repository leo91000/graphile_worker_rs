use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use syn::{parse_macro_input, LitStr};

const STATEMENT_DELIMITER: &str = "-- graphile-worker-rs:statement";
const BREAKING_MARKER: &str = "-- graphile-worker-rs:breaking";
const METADATA_PREFIX: &str = "-- graphile-worker-rs:";

#[proc_macro]
pub fn include_migrations(input: TokenStream) -> TokenStream {
    let relative_dir = parse_macro_input!(input as LitStr);

    match expand_include_migrations(&relative_dir.value()) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_include_migrations(relative_dir: &str) -> Result<proc_macro2::TokenStream, syn::Error> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| error("CARGO_MANIFEST_DIR is not set for include_migrations!"))?;
    let migration_dir = Path::new(&manifest_dir).join(relative_dir);
    let migrations = load_migrations(&migration_dir)?;

    let tracked_sources = migrations.iter().map(|migration| {
        let path = LitStr::new(
            &format!(
                "{}/{}.sql",
                relative_dir.trim_end_matches('/'),
                migration.name
            ),
            Span::call_site(),
        );

        quote! {
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", #path))
        }
    });

    let migrations = migrations.iter().map(|migration| {
        let name = LitStr::new(&migration.name, Span::call_site());
        let is_breaking = migration.is_breaking;
        let statements = migration
            .statements
            .iter()
            .map(|statement| LitStr::new(statement, Span::call_site()));

        quote! {
            ::graphile_worker_migrations_core::GraphileWorkerMigration {
                name: #name,
                is_breaking: #is_breaking,
                stmts: &[#(#statements),*],
            }
        }
    });

    Ok(quote! {
        {
            const _GRAPHILE_WORKER_MIGRATION_SQL_FILES: &[&str] = &[#(#tracked_sources),*];
            &[#(#migrations),*]
        }
    })
}

fn load_migrations(migration_dir: &Path) -> Result<Vec<Migration>, syn::Error> {
    let entries = fs::read_dir(migration_dir).map_err(|source| {
        error(format!(
            "failed to read migration directory `{}`: {source}",
            migration_dir.display()
        ))
    })?;

    let mut migrations = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| {
            error(format!(
                "failed to read an entry in migration directory `{}`: {source}",
                migration_dir.display()
            ))
        })?;
        let path = entry.path();

        if !path.is_file()
            || path.extension().and_then(|extension| extension.to_str()) != Some("sql")
        {
            continue;
        }

        migrations.push(load_migration_file(path)?);
    }

    sort_and_validate_migrations(migrations)
}

fn load_migration_file(path: PathBuf) -> Result<Migration, syn::Error> {
    let file_name = path
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .ok_or_else(|| {
            error(format!(
                "migration path `{}` has no UTF-8 filename",
                path.display()
            ))
        })?;
    let number = parse_migration_number(file_name)?;
    let script = fs::read_to_string(&path).map_err(|source| {
        error(format!(
            "failed to read migration `{}`: {source}",
            path.display()
        ))
    })?;
    let parsed = parse_migration_script(&script);

    if parsed.statements.is_empty() {
        return Err(error(format!(
            "migration `{}` does not contain any SQL statements",
            path.display()
        )));
    }

    Ok(Migration {
        number,
        name: format!("m{number:06}"),
        is_breaking: parsed.is_breaking,
        statements: parsed.statements,
    })
}

fn parse_migration_number(file_name: &str) -> Result<u32, syn::Error> {
    if file_name.len() != "m000001.sql".len()
        || !file_name.starts_with('m')
        || !file_name.ends_with(".sql")
    {
        return Err(error(format!(
            "migration filename `{file_name}` must match `mNNNNNN.sql`"
        )));
    }

    let number = &file_name[1..7];
    if !number.as_bytes().iter().all(u8::is_ascii_digit) {
        return Err(error(format!(
            "migration filename `{file_name}` must contain six digits"
        )));
    }

    number.parse().map_err(|source| {
        error(format!(
            "failed to parse migration number from `{file_name}`: {source}"
        ))
    })
}

fn parse_migration_script(script: &str) -> ParsedMigrationScript {
    let is_breaking = script.lines().any(|line| line.trim() == BREAKING_MARKER);
    let statements = script
        .split(STATEMENT_DELIMITER)
        .filter_map(|section| {
            let statement = strip_metadata_lines(section);
            let statement = statement.trim();

            if statement.is_empty() {
                return None;
            }

            Some(statement.to_owned())
        })
        .collect();

    ParsedMigrationScript {
        is_breaking,
        statements,
    }
}

fn strip_metadata_lines(section: &str) -> String {
    section
        .lines()
        .filter(|line| !line.trim().starts_with(METADATA_PREFIX))
        .collect::<Vec<_>>()
        .join("\n")
}

fn sort_and_validate_migrations(
    mut migrations: Vec<Migration>,
) -> Result<Vec<Migration>, syn::Error> {
    if migrations.is_empty() {
        return Err(error("no migration SQL files found"));
    }

    migrations.sort_by_key(|migration| migration.number);

    for (index, migration) in migrations.iter().enumerate() {
        if index > 0 && migrations[index - 1].number == migration.number {
            return Err(error(format!(
                "duplicate migration number m{:06}",
                migration.number
            )));
        }

        let expected = index as u32 + 1;
        if migration.number != expected {
            return Err(error(format!(
                "migration numbers must be contiguous from m000001; expected m{expected:06}, found m{:06}",
                migration.number
            )));
        }
    }

    Ok(migrations)
}

fn error(message: impl Into<String>) -> syn::Error {
    syn::Error::new(Span::call_site(), message.into())
}

#[derive(Debug)]
struct Migration {
    number: u32,
    name: String,
    is_breaking: bool,
    statements: Vec<String>,
}

struct ParsedMigrationScript {
    is_breaking: bool,
    statements: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn migration(number: u32) -> Migration {
        Migration {
            number,
            name: format!("m{number:06}"),
            is_breaking: false,
            statements: vec!["select 1".to_owned()],
        }
    }

    #[test]
    fn parses_valid_migration_filenames() {
        assert_eq!(parse_migration_number("m000001.sql").unwrap(), 1);
        assert_eq!(parse_migration_number("m123456.sql").unwrap(), 123456);
    }

    #[test]
    fn rejects_invalid_migration_filenames() {
        assert!(parse_migration_number("000001.sql").is_err());
        assert!(parse_migration_number("m00001.sql").is_err());
        assert!(parse_migration_number("m000001_extra.sql").is_err());
        assert!(parse_migration_number("m00000x.sql").is_err());
    }

    #[test]
    fn sorts_migrations_by_number() {
        let migrations = sort_and_validate_migrations(vec![migration(2), migration(1)]).unwrap();

        assert_eq!(migrations[0].number, 1);
        assert_eq!(migrations[1].number, 2);
    }

    #[test]
    fn rejects_non_contiguous_migrations() {
        let error = sort_and_validate_migrations(vec![migration(1), migration(3)]).unwrap_err();

        assert!(error
            .to_string()
            .contains("migration numbers must be contiguous"));
    }

    #[test]
    fn detects_breaking_marker_and_removes_metadata() {
        let parsed = parse_migration_script(
            "\n-- graphile-worker-rs:breaking\n\nselect 1;\n-- normal comment\n",
        );

        assert!(parsed.is_breaking);
        assert_eq!(parsed.statements, vec!["select 1;\n-- normal comment"]);
    }

    #[test]
    fn splits_statements_and_skips_empty_sections() {
        let parsed = parse_migration_script(
            "\nselect 1;\n-- graphile-worker-rs:statement\n\n-- graphile-worker-rs:statement\nselect 2;\n",
        );

        assert_eq!(parsed.statements, vec!["select 1;", "select 2;"]);
    }

    #[test]
    fn empty_script_has_no_statements() {
        let parsed = parse_migration_script("\n-- graphile-worker-rs:breaking\n\n");

        assert!(parsed.statements.is_empty());
    }
}
