//! Loading and validating `vump.toml`.
//!
//! Configuration is plain data: it is read once at the edge of the program and
//! passed into use cases by value. It deliberately has no port or trait, since
//! there is nothing behavioral to substitute in a test — a test simply builds
//! the value it wants.
//!
//! Parsing is separated from discovery so the interesting part, validation,
//! needs no filesystem.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::domain::tag::{self, TagPattern, TagPatternError};

/// The name of the configuration file, searched for from the working directory
/// upward.
pub const FILE_NAME: &str = "vump.toml";

/// Default commit message template.
pub const DEFAULT_COMMIT_MESSAGE: &str = "chore: bump version to v{new_version}";

/// Default tag name template.
pub const DEFAULT_TAG_PATTERN: &str = "v{new_version}";

/// Default message for an annotated or signed tag.
pub const DEFAULT_TAG_MESSAGE: &str = "Release {new_version}";

/// The placeholder replaced with the newly computed version in templates.
const VERSION_PLACEHOLDER: &str = "{new_version}";

/// A validated configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Git integration settings.
    pub git: GitSettings,
    /// Every project declared in this repository.
    ///
    /// A single-project repository normalizes to exactly one unnamed project,
    /// so downstream code never has to special-case the two forms.
    pub projects: Vec<Project>,
}

/// One independently-versioned unit within a repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// `None` for a single-project repository.
    pub name: Option<String>,
    /// Version files, as paths relative to the directory holding `vump.toml`.
    pub files: Vec<String>,
    /// Tag template for this project, overriding the repository-wide one.
    ///
    /// Independently-versioned projects need distinct tags: without this they
    /// would all be tagged `v1.2.3`, colliding the moment two of them reach the
    /// same version, and leaving no way to tell which project a pushed tag
    /// refers to.
    pub tag_pattern: Option<String>,
}

/// How a tag object is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TagStyle {
    /// A tag object carrying a message, a tagger, and a date.
    ///
    /// The default. `git describe` prefers annotated tags and some release
    /// tooling ignores lightweight ones outright — a release tag is the case
    /// the annotated format exists for.
    #[default]
    Annotated,
    /// A bare pointer to a commit, with no tagger, date, or message.
    Lightweight,
    /// An annotated tag, signed. Requires a signing key git can reach.
    Signed,
}

impl TagStyle {
    /// The name this style carries in configuration.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Annotated => "annotated",
            Self::Lightweight => "lightweight",
            Self::Signed => "signed",
        }
    }
}

/// Git integration settings.
///
/// These are decisions the user has already made. When a setting is present it
/// is acted upon, never re-asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSettings {
    /// Stage and commit the changed files.
    pub commit: bool,
    /// Template for the commit message.
    pub commit_message: String,
    /// Create a tag. Implies `commit`.
    pub tag: bool,
    /// Template for the tag name.
    pub tag_pattern: String,
    /// How the tag object is written.
    pub tag_style: TagStyle,
    /// Template for the message carried by an annotated or signed tag.
    pub tag_message: String,
    /// Push the commit and tag to the remote. Implies `commit`.
    pub push: bool,
}

impl Default for GitSettings {
    fn default() -> Self {
        Self {
            commit: false,
            commit_message: DEFAULT_COMMIT_MESSAGE.to_owned(),
            tag: false,
            tag_pattern: DEFAULT_TAG_PATTERN.to_owned(),
            tag_style: TagStyle::default(),
            tag_message: DEFAULT_TAG_MESSAGE.to_owned(),
            push: false,
        }
    }
}

/// Why configuration could not be loaded or is not usable.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConfigError {
    /// No `vump.toml` exists in the starting directory or any ancestor.
    #[error(
        "no {FILE_NAME} found in {start} or any parent directory; run `vump init` to create one"
    )]
    NotFound {
        /// Directory the upward search started from.
        start: PathBuf,
    },

    /// The file exists but could not be read.
    #[error("cannot read {path}: {detail}")]
    Unreadable {
        /// Path that could not be read.
        path: PathBuf,
        /// Underlying I/O error.
        detail: String,
    },

    /// The file is not valid TOML, or does not match the expected shape.
    #[error("{path} is not valid: {detail}")]
    Invalid {
        /// Path that failed to parse.
        path: PathBuf,
        /// Parser-supplied detail.
        detail: String,
    },

    /// Neither a top-level `files` list nor any `[[project]]` was declared.
    #[error(
        "{path} declares no files; add `files = [\"VERSION\"]`, or one or more [[project]] entries"
    )]
    NoFiles {
        /// Path of the offending configuration.
        path: PathBuf,
    },

    /// Both a top-level `files` list and `[[project]]` entries were declared.
    #[error(
        "{path} mixes a top-level `files` list with [[project]] entries; use one form or the other"
    )]
    MixedForms {
        /// Path of the offending configuration.
        path: PathBuf,
    },

    /// A project was declared without a usable name.
    #[error("{path} has a [[project]] with an empty name")]
    UnnamedProject {
        /// Path of the offending configuration.
        path: PathBuf,
    },

    /// Two projects share a name, making `--project` ambiguous.
    #[error("{path} declares more than one project named {name:?}")]
    DuplicateProject {
        /// Path of the offending configuration.
        path: PathBuf,
        /// The repeated name.
        name: String,
    },

    /// A project has an empty file list.
    #[error("{path} declares project {name:?} with no files")]
    EmptyProject {
        /// Path of the offending configuration.
        path: PathBuf,
        /// The project with no files.
        name: String,
    },

    /// `--project` named something that is not declared.
    #[error("no project named {name:?}; declared projects are: {available}")]
    UnknownProject {
        /// The requested name.
        name: String,
        /// Comma-separated list of declared names.
        available: String,
    },

    /// Several projects exist and none was selected.
    #[error("this repository declares several projects; select one with --project <{available}>")]
    ProjectRequired {
        /// Pipe-separated list of declared names.
        available: String,
    },

    /// `--project` was given for a repository that declares no named projects.
    #[error("{FILE_NAME} declares a single project, so --project {name:?} does not apply")]
    ProjectNotApplicable {
        /// The requested name.
        name: String,
    },

    /// A tag pattern names a project in a repository that has none.
    #[error(
        "tag pattern {pattern:?} refers to a project name, but this repository declares a single unnamed project"
    )]
    ProjectPlaceholderUnavailable {
        /// The offending pattern.
        pattern: String,
    },

    /// A tag pattern cannot be used to name or read tags.
    #[error("{}: {source}", match project {
        Some(name) => format!("project {name:?}"),
        None => "[git].tag_pattern".to_owned(),
    })]
    UnusableTagPattern {
        /// The project the pattern belongs to, if it is not the shared one.
        project: Option<String>,
        /// What is wrong with it.
        source: TagPatternError,
    },

    /// Several projects claim the same tag.
    #[error(
        "tag {tag:?} matches more than one project ({projects}); give each a distinct tag_pattern, or select one with --project"
    )]
    AmbiguousTag {
        /// The tag that could not be attributed.
        tag: String,
        /// Comma-separated names of the projects that matched.
        projects: String,
    },
}

impl Config {
    /// Parses and validates configuration from TOML text.
    ///
    /// `path` is used only to label diagnostics.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] when the text is not valid TOML or the
    /// declared shape is unusable.
    pub fn parse(path: &Path, text: &str) -> Result<Self, ConfigError> {
        let raw: RawConfig = toml::from_str(text).map_err(|e| ConfigError::Invalid {
            path: path.to_path_buf(),
            detail: e.message().to_owned(),
        })?;

        if !raw.files.is_empty() && !raw.project.is_empty() {
            return Err(ConfigError::MixedForms {
                path: path.to_path_buf(),
            });
        }

        let projects = if raw.project.is_empty() {
            if raw.files.is_empty() {
                return Err(ConfigError::NoFiles {
                    path: path.to_path_buf(),
                });
            }
            vec![Project {
                name: None,
                files: raw.files,
                tag_pattern: None,
            }]
        } else {
            let mut seen: Vec<String> = Vec::new();
            let mut projects = Vec::with_capacity(raw.project.len());
            for entry in raw.project {
                let name = entry.name.trim().to_owned();
                if name.is_empty() {
                    return Err(ConfigError::UnnamedProject {
                        path: path.to_path_buf(),
                    });
                }
                if seen.contains(&name) {
                    return Err(ConfigError::DuplicateProject {
                        path: path.to_path_buf(),
                        name,
                    });
                }
                if entry.files.is_empty() {
                    return Err(ConfigError::EmptyProject {
                        path: path.to_path_buf(),
                        name,
                    });
                }
                seen.push(name.clone());
                projects.push(Project {
                    name: Some(name),
                    files: entry.files,
                    tag_pattern: entry.tag_pattern,
                });
            }
            projects
        };

        Ok(Self {
            git: raw.git.into(),
            projects,
        })
    }

    /// Finds the nearest `vump.toml`, searching `start` and then each ancestor.
    ///
    /// Returns the directory containing the file alongside the parsed
    /// configuration. File paths in the configuration are relative to that
    /// directory, not to the working directory.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] when no configuration exists above `start`, or
    /// when the one found is unreadable or invalid.
    pub fn discover(start: &Path) -> Result<(PathBuf, Self), ConfigError> {
        for dir in start.ancestors() {
            let candidate = dir.join(FILE_NAME);
            if !candidate.is_file() {
                continue;
            }
            let text =
                std::fs::read_to_string(&candidate).map_err(|e| ConfigError::Unreadable {
                    path: candidate.clone(),
                    detail: e.to_string(),
                })?;
            let config = Self::parse(&candidate, &text)?;
            return Ok((dir.to_path_buf(), config));
        }

        Err(ConfigError::NotFound {
            start: start.to_path_buf(),
        })
    }

    /// Selects the project to operate on.
    ///
    /// A repository with one unnamed project needs no selection. A repository
    /// with several named projects requires one, because guessing which the
    /// caller meant is exactly the ambiguity that named projects exist to
    /// remove.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] when the requested project does not exist, or
    /// when several exist and none was requested.
    pub fn select(&self, requested: Option<&str>) -> Result<&Project, ConfigError> {
        match requested {
            Some(name) => self
                .projects
                .iter()
                .find(|p| p.name.as_deref() == Some(name))
                .ok_or_else(|| {
                    if self.is_single_unnamed() {
                        ConfigError::ProjectNotApplicable {
                            name: name.to_owned(),
                        }
                    } else {
                        ConfigError::UnknownProject {
                            name: name.to_owned(),
                            available: self.project_names(", "),
                        }
                    }
                }),
            None if self.projects.len() == 1 => Ok(&self.projects[0]),
            None => Err(ConfigError::ProjectRequired {
                available: self.project_names("|"),
            }),
        }
    }

    /// Whether this configuration describes a single, unnamed project.
    #[must_use]
    pub fn is_single_unnamed(&self) -> bool {
        matches!(self.projects.as_slice(), [only] if only.name.is_none())
    }

    /// The tag template for `project`, with the project name substituted.
    ///
    /// A project's own pattern wins over the repository-wide one, which is what
    /// lets independently-versioned projects carry distinguishable tags.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] when the resulting pattern is unusable — it
    /// names a project in a repository whose single project is unnamed, or it
    /// does not place the version exactly once.
    pub fn tag_pattern_for(&self, project: &Project) -> Result<TagPattern, ConfigError> {
        let template = project
            .tag_pattern
            .as_deref()
            .unwrap_or(&self.git.tag_pattern);

        let resolved =
            tag::apply_project_name(template, project.name.as_deref()).ok_or_else(|| {
                ConfigError::ProjectPlaceholderUnavailable {
                    pattern: template.to_owned(),
                }
            })?;

        TagPattern::parse(&resolved).map_err(|source| ConfigError::UnusableTagPattern {
            project: project.name.clone(),
            source,
        })
    }

    /// Finds the project a tag belongs to, and the version it claims.
    ///
    /// This is what lets CI verify a pushed tag without being told separately
    /// which project it refers to.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] when more than one project claims the tag,
    /// which means their patterns do not distinguish them.
    pub fn project_for_tag(
        &self,
        tag: &str,
    ) -> Result<Option<(&Project, semver::Version)>, ConfigError> {
        let mut matched: Vec<(&Project, semver::Version)> = Vec::new();

        for project in &self.projects {
            // A project whose pattern is unusable is skipped rather than
            // failing the lookup: the error belongs to whichever command
            // actually needs that project.
            let Ok(pattern) = self.tag_pattern_for(project) else {
                continue;
            };
            if let Some(version) = pattern.extract(tag) {
                matched.push((project, version));
            }
        }

        match matched.len() {
            0 => Ok(None),
            1 => Ok(matched.into_iter().next()),
            _ => Err(ConfigError::AmbiguousTag {
                tag: tag.to_owned(),
                projects: matched
                    .iter()
                    .filter_map(|(p, _)| p.name.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            }),
        }
    }

    /// The tag templates every project would produce, for diagnostics.
    ///
    /// Patterns that cannot be resolved are omitted: this exists to help a
    /// reader recognize their mistake, not to report a second one.
    #[must_use]
    pub fn tag_patterns(&self) -> Vec<String> {
        self.projects
            .iter()
            .filter_map(|project| {
                let template = project
                    .tag_pattern
                    .as_deref()
                    .unwrap_or(&self.git.tag_pattern);
                tag::apply_project_name(template, project.name.as_deref())
            })
            .collect()
    }

    /// Declared project names joined by `separator`.
    #[must_use]
    pub fn project_names(&self, separator: &str) -> String {
        let mut out = String::new();
        for (i, name) in self
            .projects
            .iter()
            .filter_map(|p| p.name.as_deref())
            .enumerate()
        {
            if i > 0 {
                out.push_str(separator);
            }
            // Writing to a String is infallible.
            let _ = write!(out, "{name}");
        }
        out
    }
}

impl GitSettings {
    /// Renders a template, substituting the version and the project name.
    ///
    /// `{project}` is available here as well as in tag patterns: in a
    /// multi-project repository a message saying only "bump version to v1.0.1"
    /// does not say *whose* version moved, and every project's commits read
    /// identically.
    ///
    /// A `{project}` in a repository whose single project is unnamed has
    /// nothing to stand for and is dropped, leaving the surrounding text.
    #[must_use]
    pub fn render(template: &str, project: Option<&str>, version: &semver::Version) -> String {
        template
            .replace(tag::PROJECT_PLACEHOLDER, project.unwrap_or_default())
            .replace(VERSION_PLACEHOLDER, &version.to_string())
    }

    /// The commit message for `version`.
    #[must_use]
    pub fn commit_message_for(&self, project: Option<&str>, version: &semver::Version) -> String {
        Self::render(&self.commit_message, project, version)
    }
}

/// Wire format of `vump.toml`.
///
/// Unknown fields are rejected so that a typo in a setting name surfaces as an
/// error instead of being silently ignored.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    git: RawGit,
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    project: Vec<RawProject>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProject {
    name: String,
    #[serde(default)]
    files: Vec<String>,
    tag_pattern: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGit {
    commit: Option<bool>,
    commit_message: Option<String>,
    tag: Option<bool>,
    tag_pattern: Option<String>,
    tag_style: Option<TagStyle>,
    tag_message: Option<String>,
    push: Option<bool>,
}

impl From<RawGit> for GitSettings {
    fn from(raw: RawGit) -> Self {
        let defaults = Self::default();
        Self {
            commit: raw.commit.unwrap_or(defaults.commit),
            commit_message: raw.commit_message.unwrap_or(defaults.commit_message),
            tag: raw.tag.unwrap_or(defaults.tag),
            tag_pattern: raw.tag_pattern.unwrap_or(defaults.tag_pattern),
            tag_style: raw.tag_style.unwrap_or(defaults.tag_style),
            tag_message: raw.tag_message.unwrap_or(defaults.tag_message),
            push: raw.push.unwrap_or(defaults.push),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Config, ConfigError> {
        Config::parse(Path::new("vump.toml"), text)
    }

    #[test]
    fn tags_are_annotated_unless_configured_otherwise() {
        // A release tag is what the annotated format exists for, so it is the
        // default rather than the opt-in.
        assert_eq!(GitSettings::default().tag_style, TagStyle::Annotated);
        assert_eq!(GitSettings::default().tag_message, DEFAULT_TAG_MESSAGE);
    }

    #[test]
    fn a_tag_style_is_read_from_configuration() {
        for (written, expected) in [
            ("lightweight", TagStyle::Lightweight),
            ("annotated", TagStyle::Annotated),
            ("signed", TagStyle::Signed),
        ] {
            let raw: RawGit = toml::from_str(&format!("tag_style = \"{written}\"")).unwrap();
            assert_eq!(GitSettings::from(raw).tag_style, expected);
        }
    }

    #[test]
    fn an_unknown_tag_style_is_rejected() {
        assert!(toml::from_str::<RawGit>("tag_style = \"gpg\"").is_err());
    }

    #[test]
    fn single_project_needs_only_a_file_list() {
        let cfg = parse(r#"files = ["VERSION"]"#).unwrap();
        assert!(cfg.is_single_unnamed());
        assert_eq!(cfg.projects[0].files, ["VERSION"]);
        // Absent git settings fall back to doing nothing.
        assert_eq!(cfg.git, GitSettings::default());
    }

    #[test]
    fn named_projects_are_preserved_in_order() {
        let cfg = parse(
            r#"
            [[project]]
            name = "api"
            files = ["services/api/Cargo.toml"]

            [[project]]
            name = "web"
            files = ["apps/web/package.json"]
            "#,
        )
        .unwrap();

        assert!(!cfg.is_single_unnamed());
        assert_eq!(cfg.project_names(", "), "api, web");
        assert_eq!(
            cfg.select(Some("web")).unwrap().files,
            ["apps/web/package.json"]
        );
    }

    #[test]
    fn git_settings_are_read_and_defaulted_individually() {
        let cfg = parse(
            r#"
            files = ["VERSION"]
            [git]
            tag = true
            commit_message = "release {new_version}"
            "#,
        )
        .unwrap();

        assert!(cfg.git.tag);
        assert!(!cfg.git.commit, "unset booleans stay false");
        assert_eq!(cfg.git.commit_message, "release {new_version}");
        // An unset template still gets its default rather than an empty string.
        assert_eq!(cfg.git.tag_pattern, DEFAULT_TAG_PATTERN);
    }

    #[test]
    fn templates_render_the_new_version() {
        let git = GitSettings::default();
        let version: semver::Version = "1.2.3-rc.1".parse().unwrap();
        assert_eq!(
            git.commit_message_for(None, &version),
            "chore: bump version to v1.2.3-rc.1"
        );
    }

    #[test]
    fn a_commit_message_can_name_the_project() {
        // Without this, every project's bump commit in a monorepo reads
        // identically and says nothing about whose version moved.
        let git = GitSettings {
            commit_message: "chore({project}): release {new_version}".to_owned(),
            ..GitSettings::default()
        };
        let version: semver::Version = "2.0.0".parse().unwrap();

        assert_eq!(
            git.commit_message_for(Some("api"), &version),
            "chore(api): release 2.0.0"
        );
    }

    #[test]
    fn naming_a_project_that_has_none_leaves_the_rest_of_the_message() {
        // A single-project repository has no name to substitute. Dropping the
        // placeholder is better than refusing a commit over cosmetics.
        let git = GitSettings {
            commit_message: "bump {project} to {new_version}".to_owned(),
            ..GitSettings::default()
        };
        let version: semver::Version = "1.0.0".parse().unwrap();

        assert_eq!(git.commit_message_for(None, &version), "bump  to 1.0.0");
    }

    #[test]
    fn declaring_nothing_is_rejected() {
        assert!(matches!(
            parse("").unwrap_err(),
            ConfigError::NoFiles { .. }
        ));
    }

    #[test]
    fn mixing_both_forms_is_rejected() {
        let err = parse(
            r#"
            files = ["VERSION"]
            [[project]]
            name = "api"
            files = ["api/Cargo.toml"]
            "#,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::MixedForms { .. }));
    }

    #[test]
    fn duplicate_project_names_are_rejected() {
        let err = parse(
            r#"
            [[project]]
            name = "api"
            files = ["a/Cargo.toml"]

            [[project]]
            name = "api"
            files = ["b/Cargo.toml"]
            "#,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::DuplicateProject { .. }));
    }

    #[test]
    fn a_project_without_files_is_rejected() {
        let err = parse("[[project]]\nname = \"api\"\n").unwrap_err();
        assert!(matches!(err, ConfigError::EmptyProject { .. }));
    }

    #[test]
    fn an_unnamed_project_is_rejected() {
        let err = parse("[[project]]\nname = \"  \"\nfiles = [\"VERSION\"]\n").unwrap_err();
        assert!(matches!(err, ConfigError::UnnamedProject { .. }));
    }

    #[test]
    fn a_misspelled_setting_is_rejected_rather_than_ignored() {
        // Silently ignoring `comit` would mean the user believes git integration
        // is on while it is off.
        let err = parse("files = [\"VERSION\"]\n[git]\ncomit = true\n").unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }));
    }

    #[test]
    fn selecting_among_several_projects_is_mandatory() {
        let cfg = parse(
            r#"
            [[project]]
            name = "api"
            files = ["a/Cargo.toml"]

            [[project]]
            name = "web"
            files = ["b/package.json"]
            "#,
        )
        .unwrap();

        assert!(matches!(
            cfg.select(None).unwrap_err(),
            ConfigError::ProjectRequired { .. }
        ));
        assert!(matches!(
            cfg.select(Some("nope")).unwrap_err(),
            ConfigError::UnknownProject { .. }
        ));
    }

    // ─── Tag patterns ────────────────────────────────────────────────────────

    const MULTI: &str = r#"
        [[project]]
        name = "api"
        files = ["api/Cargo.toml"]
        tag_pattern = "api-v{new_version}"

        [[project]]
        name = "web"
        files = ["web/package.json"]
        tag_pattern = "web-v{new_version}"
    "#;

    fn v(text: &str) -> semver::Version {
        text.parse().unwrap()
    }

    #[test]
    fn a_project_pattern_overrides_the_repository_wide_one() {
        let cfg = parse(MULTI).unwrap();
        let api = cfg.select(Some("api")).unwrap();

        assert_eq!(
            cfg.tag_pattern_for(api).unwrap().render(&v("1.2.3")),
            "api-v1.2.3"
        );
    }

    #[test]
    fn a_project_without_its_own_pattern_falls_back() {
        let cfg = parse(
            r#"
            [git]
            tag_pattern = "release-{new_version}"

            [[project]]
            name = "api"
            files = ["api/Cargo.toml"]
            "#,
        )
        .unwrap();

        let api = cfg.select(Some("api")).unwrap();
        assert_eq!(
            cfg.tag_pattern_for(api).unwrap().render(&v("1.0.0")),
            "release-1.0.0"
        );
    }

    #[test]
    fn one_repository_wide_pattern_can_name_every_project() {
        // The {project} placeholder avoids repeating a pattern per project.
        let cfg = parse(
            r#"
            [git]
            tag_pattern = "{project}-v{new_version}"

            [[project]]
            name = "api"
            files = ["api/Cargo.toml"]

            [[project]]
            name = "web"
            files = ["web/package.json"]
            "#,
        )
        .unwrap();

        for (name, expected) in [("api", "api-v2.0.0"), ("web", "web-v2.0.0")] {
            let project = cfg.select(Some(name)).unwrap();
            assert_eq!(
                cfg.tag_pattern_for(project).unwrap().render(&v("2.0.0")),
                expected
            );
        }
    }

    #[test]
    fn naming_a_project_that_has_no_name_is_rejected() {
        let cfg = parse(
            r#"
            files = ["VERSION"]
            [git]
            tag_pattern = "{project}-v{new_version}"
            "#,
        )
        .unwrap();

        let err = cfg.tag_pattern_for(&cfg.projects[0]).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::ProjectPlaceholderUnavailable { .. }
        ));
    }

    #[test]
    fn a_pattern_without_a_version_is_rejected() {
        let cfg = parse("files = [\"VERSION\"]\n[git]\ntag_pattern = \"release\"\n").unwrap();
        let err = cfg.tag_pattern_for(&cfg.projects[0]).unwrap_err();
        assert!(matches!(err, ConfigError::UnusableTagPattern { .. }));
    }

    #[test]
    fn a_tag_identifies_the_project_that_produced_it() {
        let cfg = parse(MULTI).unwrap();

        let (project, version) = cfg.project_for_tag("api-v1.2.3").unwrap().unwrap();
        assert_eq!(project.name.as_deref(), Some("api"));
        assert_eq!(version, v("1.2.3"));

        let (project, version) = cfg.project_for_tag("web-v4.5.6-rc.1").unwrap().unwrap();
        assert_eq!(project.name.as_deref(), Some("web"));
        assert_eq!(version, v("4.5.6-rc.1"));
    }

    #[test]
    fn a_tag_no_project_produces_matches_nothing() {
        let cfg = parse(MULTI).unwrap();
        // A bare version is not a tag this repository would create; the caller
        // falls back to selecting a project explicitly.
        assert!(cfg.project_for_tag("1.2.3").unwrap().is_none());
        assert!(cfg.project_for_tag("other-v1.2.3").unwrap().is_none());
    }

    #[test]
    fn projects_sharing_a_pattern_cannot_attribute_a_tag() {
        // This is the collision per-project patterns exist to prevent, and it
        // must be reported rather than guessed at.
        let cfg = parse(
            r#"
            [[project]]
            name = "api"
            files = ["api/Cargo.toml"]

            [[project]]
            name = "web"
            files = ["web/package.json"]
            "#,
        )
        .unwrap();

        let err = cfg.project_for_tag("v1.2.3").unwrap_err();
        assert!(matches!(err, ConfigError::AmbiguousTag { .. }));
    }

    #[test]
    fn a_single_project_repository_still_recognizes_its_own_tags() {
        let cfg = parse(r#"files = ["VERSION"]"#).unwrap();

        let (project, version) = cfg.project_for_tag("v1.2.3").unwrap().unwrap();
        assert!(project.name.is_none());
        assert_eq!(version, v("1.2.3"));
    }

    #[test]
    fn selecting_a_project_in_a_single_project_repo_is_reported_clearly() {
        let cfg = parse(r#"files = ["VERSION"]"#).unwrap();
        // The generic "unknown project" wording would be misleading here.
        assert!(matches!(
            cfg.select(Some("api")).unwrap_err(),
            ConfigError::ProjectNotApplicable { .. }
        ));
        assert!(cfg.select(None).is_ok());
    }
}
