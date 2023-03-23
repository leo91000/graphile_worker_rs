use std::{
    collections::HashMap,
    env,
    fs::{self, OpenOptions},
    io::prelude::*,
    path::PathBuf,
    process::{exit, Command, Stdio},
    str::FromStr,
};

use clap::ValueEnum;
use derive_more::Display;
use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use structstruck::strike;

use crate::utils::StringUtils;

#[derive(ValueEnum, Clone, Display)]
pub enum ReleaseType {
    Patch,
    Minor,
    Major,
    Auto,
}

/// Input 1 : Update type (fix, minor, major, auto)
/// ====================================
/// 1. Dependency graph of workspaces
/// ====================================
/// 2. From deeper workspace to higher one :
/// - Get all commit in workspace from latest tag {lib_name}@{version}
/// - If no commit found, continue;
/// - If auto mode, figure out if fix, minor or major with conventional commits
/// - Bump version in Cargo.toml
/// - Generate CHANGELOG.md from changes
/// - Add to list of commit tag {lib_name}@{version}
/// - Add package to list of packages to publish (FIFO)
/// ====================================
/// 3. Commit all changes and tag with all bumped tags
/// ====================================
/// 4. cd into all packages to release (FIFO) & cargo publish
pub fn release_command(release_type: ReleaseType) {
    if is_git_directory_dirty() {
        println!("\n\nGit directory is dirty, please commit all changes before releasing\n");
        exit(1);
    }

    println!("Release {release_type}");
    let packages = parse_packages();

    let mut version_map = HashMap::<String, String>::new();

    for package in packages.iter().rev() {
        let package_name = &package.name;
        let tags = get_tags(Some(&format!("{package_name}@*"))).expect("Failed to find git tags");
        let mut commits =
            parse_commits(package.path.clone(), tags.get(0)).expect("Failed to parse commits");

        if commits.is_empty() {
            println!(
                "No commits found to be released for package {}",
                package_name
            );
            version_map.insert(package_name.clone(), package.version.clone());
            continue;
        }

        let release_type = FixedReleaseType::from_commits(&release_type, &commits);

        // Bump Cargo.toml package version
        let new_version = {
            let mut toml_file = toml_edit::Document::from_str(
                &fs::read_to_string(package.path.join("Cargo.toml")).unwrap(),
            )
            .expect("Failed to parse Cargo.toml");

            let parts = package
                .version
                .split('.')
                .map(|p| p.parse::<u64>())
                .collect::<Result<Vec<_>, _>>()
                .expect("Failed to parse version");
            assert_eq!(
                3,
                parts.len(),
                "Failed to parse version, expect 3 parts to version but got {}",
                parts.len()
            );
            let major = parts[0];
            let minor = parts[1];
            let patch = parts[2];

            let new_version = match release_type {
                FixedReleaseType::Patch => format!("{}.{}.{}", major, minor, patch + 1),
                FixedReleaseType::Minor => format!("{}.{}.0", major, minor + 1),
                FixedReleaseType::Major => format!("{}.0.0", major + 1),
            };

            toml_file["package"]["version"] = toml_edit::value(new_version.clone());

            for (package_name, version) in &version_map {
                if let Some(dependency) = toml_file["dependencies"].get_mut(package_name) {
                    if !dependency.is_none() {
                        dependency["version"] = toml_edit::value(version.clone());
                    }
                }
            }

            fs::write(package.path.join("Cargo.toml"), toml_file.to_string())
                .expect("Failed to write new version to package.json file");

            version_map.insert(package_name.clone(), new_version.clone());

            new_version
        };

        // Create or update CHANGELOG.md
        {
            let header = "# Changelog\n\n";
            let mut current_changelog =
                fs::read_to_string(package.path.join("CHANGELOG.md")).unwrap_or_default();

            if current_changelog.starts_with(header) {
                current_changelog.replace_range(0..header.len(), "");
            }

            let changelog = generate_changelog(&mut commits, &package.version);
            let mut file = OpenOptions::new()
                .write(true)
                .append(false)
                .create(true)
                .open(package.path.join("CHANGELOG.md"))
                .expect("Failed to open CHANGELOG file");

            writeln!(file, "{header}{changelog}\n{current_changelog}")
                .expect("Failed to write to CHANGELOG file");
        }

        // Commit changes and tag with {package_name}@{version}
        {
            let mut cmd = Command::new("git");
            cmd.current_dir(package.path.clone())
                .arg("add")
                .arg(package.path.join("Cargo.toml"))
                .arg(package.path.join("CHANGELOG.md"))
                .spawn()
                .unwrap()
                .wait()
                .expect("Failed to add changes");

            let mut cmd = Command::new("git");
            cmd.current_dir(package.path.clone())
                .arg("commit")
                .arg(package.path.join("Cargo.toml"))
                .arg(package.path.join("CHANGELOG.md"))
                .arg("-m")
                .arg(format!("chore(release): {}@{}", package.name, new_version))
                .spawn()
                .unwrap()
                .wait()
                .expect("Failed to commit changes");

            let mut cmd = Command::new("git");
            cmd.current_dir(package.path.clone())
                .arg("tag")
                .arg(format!("{}@{}", package.name, new_version))
                .spawn()
                .unwrap()
                .wait()
                .expect("Failed to tag changes");
        }

        // Publish package
        {
            let mut cmd = Command::new("cargo");
            cmd.current_dir(package.path.clone())
                .arg("publish")
                .spawn()
                .unwrap()
                .wait()
                .expect("Failed to publish package");
        }
    }

    // Push all changes with tags
    {
        let mut cmd = Command::new("git");
        cmd.arg("push")
            .spawn()
            .unwrap()
            .wait()
            .expect("Failed to push changes");

        let mut cmd = Command::new("git");
        cmd.arg("push")
            .arg("--follow-tags")
            .spawn()
            .unwrap()
            .wait()
            .expect("Failed to push changes");
    }
}

strike! {
    #[strikethrough[derive(Deserialize, Debug)]]
    pub struct CargoToml {
        package: struct {
            name: String,
            version: String,
        },
        dependencies: HashMap<
            String,
            #[serde(untagged)]
            enum {
                DependencyDetail(struct { path: Option<String> }),
                Version(String),
            }
        >
    }
}

#[derive(Deserialize, Debug)]
struct PackageDetail {
    name: String,
    version: String,
    path: PathBuf,
    nest_level: u8,
}

fn parse_packages() -> Vec<PackageDetail> {
    let dir = env::current_dir().unwrap();
    let mut packages = parse_packages_recur(dir, 0).unwrap();

    // We dedup package by name and we keep highest nest level
    packages.sort_unstable_by(|a, b| {
        let sort_a = (&a.name, a.nest_level);
        let sort_b = (&b.name, b.nest_level);
        sort_b.cmp(&sort_a)
    });
    packages.dedup_by_key(|p| p.name.clone());
    // We sort by nest level
    packages.sort_unstable_by_key(|p| p.nest_level);

    packages
}

fn parse_packages_recur(path: PathBuf, nest_level: u8) -> anyhow::Result<Vec<PackageDetail>> {
    if nest_level >= 10 {
        panic!("Nest level of dependencies reached 10. There is probably a cyclic depdency.");
    }

    let file = fs::read_to_string(path.join("Cargo.toml"))?;
    let cargo_toml: CargoToml = toml::from_str(&file)?;

    let mut packages = vec![PackageDetail {
        name: cargo_toml.package.name,
        version: cargo_toml.package.version,
        path: path.clone(),
        nest_level,
    }];

    for (_k, dependency) in cargo_toml.dependencies {
        if let Dependencies::DependencyDetail(DependencyDetail {
            path: Some(dependency_path),
        }) = dependency
        {
            let dependency_path = path.join(dependency_path);
            let mut dependency_package = parse_packages_recur(dependency_path, nest_level + 1)?;
            packages.append(&mut dependency_package);
        }
    }

    Ok(packages)
}

strike! {
    #[strikethrough[derive(Debug, Ord, Eq, PartialEq, PartialOrd, Clone)]]
    struct Commit {
        message: String,
        description: String,
        short_hash: String,
        change_type: enum CommitType {
            Feat,
            Fix,
            Chore,
            Test,
            Docs,
            Ci,
            Dev,
            Wip,
            Other(String),
        },
        breaking_change: bool,
        scope: Option<String>,
        references: Vec<struct CommitReference {
            ref_type: enum CommitReferenceType {
                PullRequest,
                Issue,
                Hash,
            },
            value: String,
        }>,
        authors: Vec<struct CommitAuthor {
            name: String,
            email: String,
        }>
    }
}

impl Commit {
    pub fn get_markdown(&self) -> String {
        format!(
            "* {}{} ([{}](https://github.com/leo91000/archimedes/commit/{2}))",
            if self.breaking_change { "🔥 " } else { "" },
            self.message,
            self.short_hash,
        )
    }
}

impl FromStr for CommitType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let r = match s {
            "fix" => Self::Fix,
            "feat" => Self::Feat,
            "chore" => Self::Chore,
            "test" => Self::Test,
            "wip" => Self::Wip,
            "docs" => Self::Docs,
            "ci" => Self::Ci,
            "dev" => Self::Dev,
            _ => Self::Other(s.to_string()),
        };

        Ok(r)
    }
}

impl CommitType {
    fn get_markdown(&self) -> String {
        match self {
            Self::Fix => "### 🐛 Fixes".to_string(),
            Self::Feat => "### ✨Features".to_string(),
            Self::Chore => "### 🧹 chores".to_string(),
            Self::Wip => "### 🚧 WIP".to_string(),
            Self::Test => "### 🧪 Tests".to_string(),
            Self::Docs => "### 📝 Docs".to_string(),
            Self::Ci => "### 🤖 CI".to_string(),
            Self::Dev => "### 🛠 Dev".to_string(),
            Self::Other(s) => format!("### 🔧 {s}"),
        }
    }
}

fn get_tags(pattern: Option<&str>) -> anyhow::Result<Vec<String>> {
    let mut cmd = Command::new("git");
    cmd.arg("--no-pager").arg("tag").arg("-l");

    if let Some(pattern) = pattern {
        cmd.arg(pattern);
    }

    cmd.arg("--sort=creatordate");

    let output = cmd.stdout(Stdio::piped()).output()?;
    let stdout: String = String::from_utf8(output.stdout)?;

    let result = stdout.lines().map(|l| l.trim().to_string()).collect();

    Ok(result)
}

// https://www.conventionalcommits.org/en/v1.0.0/
// https://regex101.com/r/FSfNvA/1
static CONVENTIONAL_COMMIT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new("(?P<type>[a-z]+)(\\((?P<scope>.+)\\))?(?P<breaking>!)?: (?P<description>.+)")
        .unwrap()
});
static CO_AUTHORED_BY_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("Co-authored-by:\\s*(?P<name>.+)(<(?P<email>.+)>)").unwrap());
static PR_RE: Lazy<Regex> = Lazy::new(|| Regex::new("\\([a-z ]*(#[0-9]+)\\s*\\)").unwrap());
static ISSUE_RE: Lazy<Regex> = Lazy::new(|| Regex::new("(#[0-9]+)").unwrap());

fn parse_commits(dir: PathBuf, from: Option<&String>) -> anyhow::Result<Vec<Commit>> {
    let mut cmd = Command::new("git");
    cmd.arg("--no-pager").arg("log");

    let to = "HEAD";
    if let Some(from) = from {
        cmd.arg(format!("{from}...{to}"));
    } else {
        cmd.arg(to);
    }

    let output = cmd
        .arg(r#"--pretty="----%n%s|%h|%an|%ae%n%b""#)
        .arg("--name-status")
        .arg("--")
        .arg(dir.to_str().unwrap())
        .stdout(Stdio::piped())
        .output()?;

    let stdout: String = String::from_utf8(output.stdout)?;

    let commits = stdout
        .split("----\n")
        .skip(1)
        .filter_map(|raw_commit| {
            let (commit_data, body) = raw_commit.split_once('\n').expect("Bad git >-<");
            let (message, short_hash, author_name, author_email) =
                commit_data.split_4("|").expect("Bad git fmt >-<");

            let cc_cap = (*CONVENTIONAL_COMMIT_RE).captures_iter(message).next()?;
            let change_type: CommitType = cc_cap.name("type")?.as_str().parse().ok()?;
            let scope = cc_cap.name("scope").map(|m| m.as_str().to_string());
            let breaking_change = cc_cap.name("breaking").is_some();
            let description = cc_cap.name("description")?.as_str();

            let mut references: Vec<CommitReference> = (*PR_RE)
                .captures_iter(description)
                .filter_map(|cap| {
                    Some(CommitReference {
                        ref_type: CommitReferenceType::PullRequest,
                        value: cap.get(1)?.as_str().into(),
                    })
                })
                .chain((*ISSUE_RE).captures_iter(description).filter_map(|cap| {
                    Some(CommitReference {
                        ref_type: CommitReferenceType::Issue,
                        value: cap.get(1)?.as_str().into(),
                    })
                }))
                .collect();

            references.push(CommitReference {
                ref_type: CommitReferenceType::Hash,
                value: short_hash.to_string(),
            });
            references.sort_by_key(|c| c.value.clone());
            references.dedup_by_key(|c| c.value.clone());

            let mut authors: Vec<CommitAuthor> = (*CO_AUTHORED_BY_RE)
                .captures_iter(body)
                .filter_map(|c| {
                    Some(CommitAuthor {
                        name: c.name("name")?.as_str().into(),
                        email: c.name("email")?.as_str().into(),
                    })
                })
                .collect();

            authors.insert(
                0,
                CommitAuthor {
                    name: author_name.to_string(),
                    email: author_email.to_string(),
                },
            );

            let description = (*PR_RE).replace_all(description, "");

            Some(Commit {
                message: message.into(),
                description: description.into(),
                short_hash: short_hash.into(),
                change_type,
                breaking_change,
                scope,
                references,
                authors,
            })
        })
        .collect::<Vec<_>>();

    Ok(commits)
}

#[derive(Debug)]
enum FixedReleaseType {
    Patch,
    Minor,
    Major,
}

impl FixedReleaseType {
    fn from_commits(release_type: &ReleaseType, commits: &[Commit]) -> Self {
        match release_type {
            ReleaseType::Patch => FixedReleaseType::Patch,
            ReleaseType::Minor => FixedReleaseType::Minor,
            ReleaseType::Major => FixedReleaseType::Major,
            ReleaseType::Auto => {
                if commits.iter().any(|c| c.breaking_change) {
                    FixedReleaseType::Major
                } else if commits.iter().any(|c| c.change_type == CommitType::Feat) {
                    FixedReleaseType::Minor
                } else {
                    FixedReleaseType::Patch
                }
            }
        }
    }
}

fn generate_changelog(commits: &mut [Commit], version: &String) -> String {
    let mut changelog = format!("## {}\n\n", version);
    println!("----------------");
    commits.sort_by_key(|c| c.change_type.clone());

    for (key, commits) in &commits.iter().group_by(|c| c.change_type.clone()) {
        println!("{:?}:", key);
        let mut group_changelog = format!("\n{}\n\n", key.get_markdown());
        let mut nb_commits = 0;
        for commit in commits {
            nb_commits += 1;
            group_changelog += format!("{}\n", commit.get_markdown()).as_str();
        }

        if nb_commits > 0 {
            changelog += group_changelog.as_str();
        }
    }

    changelog
}

fn is_git_directory_dirty() -> bool {
    let mut cmd = Command::new("git");

    let output = cmd
        .arg("status")
        .arg("--porcelain")
        .stdout(Stdio::piped())
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    !stdout.trim().is_empty()
}
