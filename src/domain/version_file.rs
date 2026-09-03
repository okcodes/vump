//! Reading and rewriting the version recorded in a project file.
//!
//! Every function here operates on file *contents* rather than on paths, which
//! keeps the format handling pure and exhaustively testable. Performing the
//! actual I/O is an adapter's job.
//!
//! The central constraint is that rewriting a version must leave the rest of
//! the file byte-for-byte identical. Key order, indentation, trailing commas
//! and comments elsewhere in the file all survive, because a tool that
//! reformats a file as a side effect of bumping it is not usable in a real
//! repository.

use std::ops::Range;

use semver::Version;
use thiserror::Error;
use toml_edit::DocumentMut;

/// Which entries of a lock file describe the project being versioned.
///
/// A lock file is the one format that can record several versions: a workspace
/// locks every member it builds, each at its own. Which of them a project means
/// is not answerable from the lock alone, so it is supplied by the caller,
/// which knows the manifests declared alongside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockScope<'a> {
    /// The only package built from this repository.
    ///
    /// The inference available when nothing names the project: a single-crate
    /// repository locks exactly one package of its own. More than one is
    /// refused rather than guessed at.
    Sole,
    /// The packages with these names, taken from the manifests declared with
    /// the lock file.
    Named(&'a [String]),
}

/// A file format vump knows how to read a version from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// npm manifest; the version lives at the top-level `version` key.
    PackageJson,
    /// Cargo manifest; the version lives at `[package].version`.
    CargoToml,
    /// Cargo lock file; the version lives in the `[[package]]` entry built
    /// from this repository rather than fetched.
    CargoLock,
    /// npm lock file; the version lives at the top level, and again under
    /// `packages[""]` from lockfile version 2 onwards.
    PackageLock,
    /// A file whose entire contents are the version.
    PlainText,
}

/// Why a version could not be read from or written to a file's contents.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VersionFileError {
    /// The filename is not one vump recognizes.
    #[error("unsupported file {name:?}; expected package.json, Cargo.toml, Cargo.lock, or VERSION")]
    UnsupportedFile {
        /// The filename that could not be classified.
        name: String,
    },

    /// The file parsed, but carries no version field.
    #[error("{file} has no {field} field")]
    MissingField {
        /// The file being read.
        file: String,
        /// A human description of the field that should have been present.
        field: String,
    },

    /// The file could not be parsed in its declared format.
    #[error("{file} is not valid {format}: {detail}")]
    Malformed {
        /// The file being read.
        file: String,
        /// The format that was expected.
        format: String,
        /// Parser-supplied detail.
        detail: String,
    },

    /// A version field was found, but is not valid semver.
    #[error("{file} contains {found:?}, which is not a valid version: {detail}")]
    InvalidVersion {
        /// The file being read.
        file: String,
        /// The raw text that failed to parse.
        found: String,
        /// Parser-supplied detail.
        detail: String,
    },

    /// The lock file records the project's version twice, and the two records
    /// disagree.
    #[error(
        "{file} records this project's version twice and the two disagree: \
         {root:?} at the top level, {nested:?} under packages[\"\"]"
    )]
    InconsistentLock {
        /// The file being read.
        file: String,
        /// The version recorded at the top level.
        root: String,
        /// The version recorded for the root package entry.
        nested: String,
    },

    /// The lock file covers several packages built from this repository, so
    /// which one records the project's version is ambiguous.
    #[error(
        "{file} covers several packages built from this repository ({members}); \
         vump cannot tell which of them records this project's version"
    )]
    WorkspaceLock {
        /// The file being read.
        file: String,
        /// Names of the local packages found, in the order they appear.
        members: String,
    },

    /// The manifest inherits its version from a workspace, so there is nothing
    /// here for vump to bump.
    #[error("{file} inherits its version from the workspace; track the workspace manifest instead")]
    InheritedVersion {
        /// The file being read.
        file: String,
    },
}

impl Format {
    /// Classifies a file by its name alone.
    ///
    /// Returns `None` when the name is not one of the recognized manifests.
    #[must_use]
    pub fn detect(file_name: &str) -> Option<Self> {
        match file_name {
            "package.json" => Some(Self::PackageJson),
            "Cargo.toml" => Some(Self::CargoToml),
            "Cargo.lock" => Some(Self::CargoLock),
            "package-lock.json" => Some(Self::PackageLock),
            "VERSION" => Some(Self::PlainText),
            _ => None,
        }
    }

    /// Classifies a file by its name, erroring when it is not recognized.
    ///
    /// # Errors
    ///
    /// Returns [`VersionFileError::UnsupportedFile`] for unrecognized names.
    pub fn require(file_name: &str) -> Result<Self, VersionFileError> {
        Self::detect(file_name).ok_or_else(|| VersionFileError::UnsupportedFile {
            name: file_name.to_owned(),
        })
    }

    /// Human-readable name of the format, used in diagnostics.
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            Self::PackageJson | Self::PackageLock => "JSON",
            Self::CargoToml | Self::CargoLock => "TOML",
            Self::PlainText => "plain text",
        }
    }

    /// Describes where the version lives, used in diagnostics.
    #[must_use]
    pub fn field_description(self) -> &'static str {
        match self {
            Self::PackageJson | Self::PackageLock => "\"version\"",
            Self::CargoToml => "[package].version",
            Self::CargoLock => "[[package]].version",
            Self::PlainText => "version",
        }
    }

    /// Whether this format is a lock file rather than a manifest.
    #[must_use]
    pub fn is_lock(self) -> bool {
        matches!(self, Self::CargoLock | Self::PackageLock)
    }

    /// Reads the version recorded in `contents`.
    ///
    /// `file` is used only to label diagnostics.
    ///
    /// # Errors
    ///
    /// Returns a [`VersionFileError`] when the contents cannot be parsed, carry
    /// no version, or carry one that is not valid semver.
    pub fn read(
        self,
        file: &str,
        contents: &str,
        scope: LockScope<'_>,
    ) -> Result<Version, VersionFileError> {
        let raw = match self {
            Self::PackageJson => {
                let span = find_package_json_version(contents).ok_or_else(|| {
                    VersionFileError::MissingField {
                        file: file.to_owned(),
                        field: self.field_description().to_owned(),
                    }
                })?;
                contents[span].to_owned()
            }
            Self::CargoToml => read_cargo_version(file, contents)?,
            Self::CargoLock => read_cargo_lock_version(file, contents, scope)?,
            Self::PackageLock => read_package_lock_version(file, contents)?,
            Self::PlainText => contents.trim().to_owned(),
        };

        if raw.is_empty() {
            return Err(VersionFileError::MissingField {
                file: file.to_owned(),
                field: self.field_description().to_owned(),
            });
        }

        raw.parse::<Version>()
            .map_err(|e| VersionFileError::InvalidVersion {
                file: file.to_owned(),
                found: raw,
                detail: e.to_string(),
            })
    }

    /// Returns `contents` with the recorded version replaced by `version`,
    /// leaving every other byte untouched.
    ///
    /// `file` is used only to label diagnostics.
    ///
    /// # Errors
    ///
    /// Returns a [`VersionFileError`] when the contents cannot be parsed or
    /// carry no version field to replace.
    pub fn write(
        self,
        file: &str,
        contents: &str,
        version: &Version,
        scope: LockScope<'_>,
    ) -> Result<String, VersionFileError> {
        match self {
            Self::PackageJson => {
                let span = find_package_json_version(contents).ok_or_else(|| {
                    VersionFileError::MissingField {
                        file: file.to_owned(),
                        field: self.field_description().to_owned(),
                    }
                })?;
                let mut out = String::with_capacity(contents.len());
                out.push_str(&contents[..span.start]);
                out.push_str(&version.to_string());
                out.push_str(&contents[span.end..]);
                Ok(out)
            }
            Self::CargoToml => write_cargo_version(file, contents, version),
            Self::CargoLock => write_cargo_lock_version(file, contents, version, scope),
            Self::PackageLock => write_package_lock_version(file, contents, version),
            // The version is the whole file, so a trailing newline is the only
            // formatting there is to preserve.
            Self::PlainText => Ok(format!("{version}\n")),
        }
    }
}

/// Locates the `[package].version` value in a Cargo manifest.
fn read_cargo_version(file: &str, contents: &str) -> Result<String, VersionFileError> {
    let doc = parse_cargo(file, contents)?;

    let package = doc
        .get("package")
        .and_then(toml_edit::Item::as_table_like)
        .ok_or_else(|| VersionFileError::MissingField {
            file: file.to_owned(),
            field: "[package]".to_owned(),
        })?;

    let version = package
        .get("version")
        .ok_or_else(|| VersionFileError::MissingField {
            file: file.to_owned(),
            field: "[package].version".to_owned(),
        })?;

    // `version.workspace = true` means the value lives in the workspace root.
    if version
        .as_table_like()
        .and_then(|t| t.get("workspace"))
        .is_some()
    {
        return Err(VersionFileError::InheritedVersion {
            file: file.to_owned(),
        });
    }

    version
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| VersionFileError::MissingField {
            file: file.to_owned(),
            field: "[package].version".to_owned(),
        })
}

/// Reads the package name a Cargo manifest declares.
///
/// This is what pairs a manifest with its entry in a shared workspace lock.
/// A virtual workspace manifest declares no package and so yields nothing.
#[must_use]
pub fn cargo_package_name(contents: &str) -> Option<String> {
    contents
        .parse::<DocumentMut>()
        .ok()?
        .get("package")?
        .as_table_like()?
        .get("name")?
        .as_str()
        .map(str::to_owned)
}

/// Rewrites `[package].version`, preserving the rest of the document.
fn write_cargo_version(
    file: &str,
    contents: &str,
    version: &Version,
) -> Result<String, VersionFileError> {
    // Reading first rejects manifests with no writable version — an inherited
    // or absent one — before any mutation is attempted.
    read_cargo_version(file, contents)?;

    let mut doc = parse_cargo(file, contents)?;
    let slot =
        doc["package"]["version"]
            .as_value_mut()
            .ok_or_else(|| VersionFileError::MissingField {
                file: file.to_owned(),
                field: "[package].version".to_owned(),
            })?;

    // Assigning through the existing value keeps its surrounding whitespace and
    // any trailing comment on the line.
    let decor = slot.decor().clone();
    *slot = toml_edit::Value::from(version.to_string());
    *slot.decor_mut() = decor;

    Ok(doc.to_string())
}

/// The path to the root package's entry, from lockfile version 2 onwards.
const NPM_ROOT_PACKAGE: &[&str] = &["packages", ""];

/// Reads the version an npm lock file records for the project itself.
fn read_package_lock_version(file: &str, contents: &str) -> Result<String, VersionFileError> {
    let root = find_json_version(contents, &[]).ok_or_else(|| VersionFileError::MissingField {
        file: file.to_owned(),
        field: "\"version\"".to_owned(),
    })?;
    let root = &contents[root];

    // Version 1 has no `packages` map. Where one exists both records describe
    // the same project, so a disagreement is a corrupt file rather than a
    // question to answer by preferring one of them.
    if let Some(nested) = find_json_version(contents, NPM_ROOT_PACKAGE) {
        let nested = &contents[nested];
        if nested != root {
            return Err(VersionFileError::InconsistentLock {
                file: file.to_owned(),
                root: root.to_owned(),
                nested: nested.to_owned(),
            });
        }
    }

    Ok(root.to_owned())
}

/// Rewrites every place an npm lock file records the project's own version.
fn write_package_lock_version(
    file: &str,
    contents: &str,
    version: &Version,
) -> Result<String, VersionFileError> {
    // Reading first rejects a lock with no version, or with two that already
    // disagree, before any mutation is attempted.
    read_package_lock_version(file, contents)?;

    let mut spans =
        vec![
            find_json_version(contents, &[]).ok_or_else(|| VersionFileError::MissingField {
                file: file.to_owned(),
                field: "\"version\"".to_owned(),
            })?,
        ];
    if let Some(nested) = find_json_version(contents, NPM_ROOT_PACKAGE) {
        spans.push(nested);
    }

    // Splicing from the end keeps the earlier spans' offsets valid.
    spans.sort_by_key(|span| std::cmp::Reverse(span.start));
    let mut out = contents.to_owned();
    for span in spans {
        out.replace_range(span, &version.to_string());
    }

    Ok(out)
}

/// Reads the version this lock file records for the project's packages.
///
/// Every entry in scope must agree. They describe one project moving as a
/// unit, so a disagreement between them is the same defect as a manifest and
/// its lock disagreeing, and is reported rather than resolved by picking one.
fn read_cargo_lock_version(
    file: &str,
    contents: &str,
    scope: LockScope<'_>,
) -> Result<String, VersionFileError> {
    let doc = parse_cargo(file, contents)?;
    let entries = local_packages(file, &doc, scope)?;

    let mut versions = entries.iter().map(|entry| entry.version.as_str());
    let first = versions.next().unwrap_or_default().to_owned();

    if let Some(other) = versions.find(|v| *v != first) {
        return Err(VersionFileError::InconsistentLock {
            file: file.to_owned(),
            root: first,
            nested: other.to_owned(),
        });
    }

    Ok(first)
}

/// Rewrites the version of every package in scope, preserving everything else.
fn write_cargo_lock_version(
    file: &str,
    contents: &str,
    version: &Version,
    scope: LockScope<'_>,
) -> Result<String, VersionFileError> {
    // Reading first rejects a lock naming none of the project's packages, or
    // recording them inconsistently, before any mutation is attempted.
    read_cargo_lock_version(file, contents, scope)?;

    let mut doc = parse_cargo(file, contents)?;
    let indexes: Vec<usize> = local_packages(file, &doc, scope)?
        .iter()
        .map(|entry| entry.index)
        .collect();

    let tables = doc
        .get_mut("package")
        .and_then(toml_edit::Item::as_array_of_tables_mut)
        .ok_or_else(|| VersionFileError::MissingField {
            file: file.to_owned(),
            field: "[[package]]".to_owned(),
        })?;

    for index in indexes {
        let slot = tables
            .get_mut(index)
            .and_then(|table| table.get_mut("version"))
            .and_then(toml_edit::Item::as_value_mut)
            .ok_or_else(|| VersionFileError::MissingField {
                file: file.to_owned(),
                field: "[[package]].version".to_owned(),
            })?;

        // Assigning through the existing value keeps its surrounding
        // whitespace and any trailing comment on the line.
        let decor = slot.decor().clone();
        *slot = toml_edit::Value::from(version.to_string());
        *slot.decor_mut() = decor;
    }

    Ok(doc.to_string())
}

/// One `[[package]]` entry that belongs to the project being versioned.
struct LocalPackage {
    index: usize,
    version: String,
}

/// Selects the `[[package]]` entries that belong to the project.
///
/// Cargo records a `source` for everything it fetched, so the entries without
/// one are exactly those built from the working tree. Which of those the
/// project means depends on the scope: a single-crate repository has one and
/// it needs no naming, while a workspace locks every member and only the
/// manifests declared alongside say which are in play.
fn local_packages(
    file: &str,
    doc: &DocumentMut,
    scope: LockScope<'_>,
) -> Result<Vec<LocalPackage>, VersionFileError> {
    let packages = doc
        .get("package")
        .and_then(toml_edit::Item::as_array_of_tables)
        .ok_or_else(|| VersionFileError::MissingField {
            file: file.to_owned(),
            field: "[[package]]".to_owned(),
        })?;

    let field = |table: &toml_edit::Table, key: &str| {
        table
            .get(key)
            .and_then(toml_edit::Item::as_str)
            .map(str::to_owned)
    };

    let local: Vec<(usize, String, String)> = packages
        .iter()
        .enumerate()
        .filter(|(_, table)| table.get("source").is_none())
        .map(|(index, table)| {
            (
                index,
                field(table, "name").unwrap_or_else(|| "unnamed".to_owned()),
                field(table, "version").unwrap_or_default(),
            )
        })
        .collect();

    let selected: Vec<LocalPackage> = match scope {
        LockScope::Sole => match local.as_slice() {
            [(index, _, version)] => vec![LocalPackage {
                index: *index,
                version: version.clone(),
            }],
            [] => Vec::new(),
            members => {
                return Err(VersionFileError::WorkspaceLock {
                    file: file.to_owned(),
                    members: members
                        .iter()
                        .map(|(_, name, _)| name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                });
            }
        },
        LockScope::Named(names) => local
            .iter()
            .filter(|(_, name, _)| names.iter().any(|wanted| wanted == name))
            .map(|(index, _, version)| LocalPackage {
                index: *index,
                version: version.clone(),
            })
            .collect(),
    };

    if selected.is_empty() {
        return Err(VersionFileError::MissingField {
            file: file.to_owned(),
            field: match scope {
                LockScope::Sole => "a [[package]] entry built from this repository".to_owned(),
                LockScope::Named(names) => {
                    format!("a [[package]] entry for {}", names.join(" or "))
                }
            },
        });
    }

    Ok(selected)
}

fn parse_cargo(file: &str, contents: &str) -> Result<DocumentMut, VersionFileError> {
    contents
        .parse::<DocumentMut>()
        .map_err(|e| VersionFileError::Malformed {
            file: file.to_owned(),
            format: "TOML".to_owned(),
            detail: e.to_string(),
        })
}

/// Returns the byte range of the *value* of the root object's `version` key.
///
/// The scan is depth-aware so that a `version` key nested inside `dependencies`
/// or any other object is never mistaken for the manifest's own version. Only a
/// string value at depth 1 qualifies.
fn find_package_json_version(contents: &str) -> Option<Range<usize>> {
    find_json_version(contents, &[])
}

/// Locates a `version` string nested under `path` in a JSON document.
///
/// An empty path means the root object. Following the path matters rather than
/// counting depth: an npm lock file puts the project's own version at
/// `packages[""].version` and every dependency's at
/// `packages["node_modules/…"].version`, which sit at the same depth and differ
/// only by which key contains them.
fn find_json_version(contents: &str, path: &[&str]) -> Option<Range<usize>> {
    let bytes = contents.as_bytes();
    let mut i = 0;
    let mut depth: i32 = 0;
    // How many leading segments of `path` the current position sits inside.
    let mut matched = 0usize;

    while i < bytes.len() {
        match bytes[i] {
            b'{' | b'[' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' => {
                depth -= 1;
                // Leaving a container the path had entered un-matches it.
                let inside = usize::try_from(depth.max(0)).ok()?;
                if inside <= matched {
                    matched = inside.saturating_sub(1);
                }
                i += 1;
            }
            b'"' => {
                let span = string_span(bytes, i)?;
                // Position just past the closing quote.
                let after = span.end + 1;
                let next = skip_whitespace(bytes, after);

                // A string followed by ':' is a key. Keys of the container the
                // path has reached sit one level below the segments matched.
                let key_depth = i32::try_from(matched).ok()? + 1;
                if next < bytes.len() && bytes[next] == b':' && depth == key_depth {
                    let key = &contents[span.clone()];
                    let value_start = skip_whitespace(bytes, next + 1);

                    if matched < path.len() {
                        if key == path[matched] {
                            matched += 1;
                        }
                    } else if key == "version" {
                        if value_start < bytes.len() && bytes[value_start] == b'"' {
                            return string_span(bytes, value_start);
                        }
                        // A non-string version is not something to rewrite.
                        return None;
                    }

                    i = value_start;
                } else {
                    i = after;
                }
            }
            _ => i += 1,
        }
    }

    None
}

/// Given the index of an opening quote, returns the range of the string's
/// contents, excluding both quotes. Honors backslash escapes.
fn string_span(bytes: &[u8], open_quote: usize) -> Option<Range<usize>> {
    let start = open_quote + 1;
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(start..i),
            _ => i += 1,
        }
    }
    None
}

fn skip_whitespace(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        text.parse().expect("test version literal must be valid")
    }

    #[test]
    fn detects_supported_filenames() {
        assert_eq!(Format::detect("package.json"), Some(Format::PackageJson));
        assert_eq!(Format::detect("Cargo.toml"), Some(Format::CargoToml));
        assert_eq!(Format::detect("VERSION"), Some(Format::PlainText));
        assert_eq!(Format::detect("pyproject.toml"), None);
        // Detection is exact; casing is not normalized.
        assert_eq!(Format::detect("cargo.toml"), None);
    }

    #[test]
    fn reads_plain_text() {
        let f = Format::PlainText;
        assert_eq!(
            f.read("VERSION", "1.2.3\n", LockScope::Sole).unwrap(),
            v("1.2.3")
        );
        assert_eq!(
            f.read("VERSION", "  1.2.3-rc.1  ", LockScope::Sole)
                .unwrap(),
            v("1.2.3-rc.1")
        );
    }

    #[test]
    fn writes_plain_text_with_trailing_newline() {
        let out = Format::PlainText
            .write("VERSION", "1.2.3\n", &v("1.3.0"), LockScope::Sole)
            .unwrap();
        assert_eq!(out, "1.3.0\n");
    }

    #[test]
    fn reads_package_json() {
        let src = r#"{"name":"app","version":"4.5.6"}"#;
        assert_eq!(
            Format::PackageJson
                .read("package.json", src, LockScope::Sole)
                .unwrap(),
            v("4.5.6")
        );
    }

    #[test]
    fn package_json_rewrite_preserves_everything_else() {
        // Deliberately unusual formatting: tabs, odd spacing, key order.
        let src = "{\n\t\"name\"  :  \"app\",\n\t\"version\":\t\"1.0.0\",\n\t\"scripts\": {\n\t\t\"build\": \"tsc\"\n\t}\n}\n";
        let out = Format::PackageJson
            .write("package.json", src, &v("1.0.1"), LockScope::Sole)
            .unwrap();

        assert_eq!(out, src.replace("\"1.0.0\"", "\"1.0.1\""));
        // Everything but the version bytes is identical.
        assert_eq!(out.replace("1.0.1", "1.0.0"), src);
    }

    #[test]
    fn package_json_ignores_nested_version_keys() {
        // A naive search-and-replace would corrupt the dependency pin here.
        let src = r#"{
  "name": "app",
  "dependencies": {
    "left-pad": { "version": "9.9.9" }
  },
  "version": "1.0.0"
}"#;
        let read = Format::PackageJson
            .read("package.json", src, LockScope::Sole)
            .unwrap();
        assert_eq!(
            read,
            v("1.0.0"),
            "must read the root version, not a nested one"
        );

        let out = Format::PackageJson
            .write("package.json", src, &v("2.0.0"), LockScope::Sole)
            .unwrap();
        assert!(
            out.contains(r#""version": "9.9.9""#),
            "nested dependency version must be untouched:\n{out}"
        );
        assert!(out.contains(r#""version": "2.0.0""#));
    }

    #[test]
    fn package_json_ignores_a_version_key_appearing_before_the_root_one() {
        // The nested key comes first in byte order, so anything that matches on
        // first-occurrence rather than depth picks the wrong one.
        let src = r#"{
  "dependencies": { "a": { "version": "0.0.1" } },
  "version": "3.0.0"
}"#;
        assert_eq!(
            Format::PackageJson
                .read("package.json", src, LockScope::Sole)
                .unwrap(),
            v("3.0.0")
        );
    }

    #[test]
    fn package_json_without_a_version_is_reported() {
        let err = Format::PackageJson
            .read("package.json", r#"{"name":"app"}"#, LockScope::Sole)
            .unwrap_err();
        assert!(matches!(err, VersionFileError::MissingField { .. }));
    }

    #[test]
    fn reads_cargo_toml() {
        let src = "[package]\nname = \"app\"\nversion = \"0.4.2\"\n";
        assert_eq!(
            Format::CargoToml
                .read("Cargo.toml", src, LockScope::Sole)
                .unwrap(),
            v("0.4.2")
        );
    }

    #[test]
    fn cargo_rewrite_preserves_comments_and_layout() {
        let src = "\
# Project manifest
[package]
name = \"app\"          # the name
version = \"0.4.2\"      # bumped by vump
edition = \"2024\"

[dependencies]
serde = \"1\"
";
        let out = Format::CargoToml
            .write("Cargo.toml", src, &v("0.5.0"), LockScope::Sole)
            .unwrap();

        assert!(out.contains("# Project manifest"));
        assert!(
            out.contains("# bumped by vump"),
            "trailing comment lost:\n{out}"
        );
        assert!(out.contains("serde = \"1\""));
        assert_eq!(out, src.replace("0.4.2", "0.5.0"));
    }

    /// npm's own output, not a hand-written approximation: these prove vump
    /// agrees with npm rather than with itself. See `testdata/README.md`.
    const NPM_LOCKS: [(u8, &str); 3] = [
        (1, include_str!("testdata/npm-lock-v1.json")),
        (2, include_str!("testdata/npm-lock-v2.json")),
        (3, include_str!("testdata/npm-lock-v3.json")),
    ];

    #[test]
    fn reads_the_project_version_from_every_npm_lock_format() {
        for (lockfile_version, contents) in NPM_LOCKS {
            assert_eq!(
                Format::PackageLock
                    .read("package-lock.json", contents, LockScope::Sole)
                    .unwrap(),
                v("1.2.3"),
                "lockfileVersion {lockfile_version}"
            );
        }
    }

    #[test]
    fn writes_every_place_an_npm_lock_records_the_project_version() {
        for (lockfile_version, contents) in NPM_LOCKS {
            let out = Format::PackageLock
                .write("package-lock.json", contents, &v("2.0.0"), LockScope::Sole)
                .unwrap();

            // Reading back is the check that matters: it fails if the two
            // records were left disagreeing.
            assert_eq!(
                Format::PackageLock
                    .read("package-lock.json", &out, LockScope::Sole)
                    .unwrap(),
                v("2.0.0"),
                "lockfileVersion {lockfile_version}"
            );
            assert!(
                !out.contains("\"1.2.3\""),
                "lockfileVersion {lockfile_version} kept the old version:\n{out}"
            );
        }
    }

    #[test]
    fn npm_lock_ignores_dependency_versions() {
        // Version 2 records the dependency's version twice, under `packages`
        // and again under `dependencies`, at the same depth as the project's
        // own entry. Only the path distinguishes them.
        for (lockfile_version, contents) in NPM_LOCKS {
            let out = Format::PackageLock
                .write("package-lock.json", contents, &v("2.0.0"), LockScope::Sole)
                .unwrap();

            assert_eq!(
                out.matches("\"2.1.3\"").count(),
                contents.matches("\"2.1.3\"").count(),
                "lockfileVersion {lockfile_version} disturbed a dependency:\n{out}"
            );
        }
    }

    #[test]
    fn an_npm_lock_stays_valid_json() {
        for (lockfile_version, contents) in NPM_LOCKS {
            let out = Format::PackageLock
                .write("package-lock.json", contents, &v("2.0.0"), LockScope::Sole)
                .unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&out)
                .unwrap_or_else(|e| panic!("lockfileVersion {lockfile_version} broke: {e}"));
            assert_eq!(parsed["version"], "2.0.0");
        }
    }

    #[test]
    fn an_npm_lock_recording_two_different_versions_is_refused() {
        // npm writes both records together, so a disagreement is a corrupt
        // file rather than a question to settle by preferring one.
        let src = r#"{
  "name": "demo",
  "version": "1.2.3",
  "lockfileVersion": 3,
  "packages": {
    "": {
      "name": "demo",
      "version": "1.2.2"
    }
  }
}"#;
        let err = Format::PackageLock
            .read("package-lock.json", src, LockScope::Sole)
            .unwrap_err();
        assert!(
            matches!(err, VersionFileError::InconsistentLock { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn an_npm_lock_is_recognized_by_name() {
        assert_eq!(
            Format::detect("package-lock.json"),
            Some(Format::PackageLock)
        );
        // yarn and pnpm record no version for the project itself.
        assert_eq!(Format::detect("yarn.lock"), None);
        assert_eq!(Format::detect("pnpm-lock.yaml"), None);
    }

    /// A lock file as Cargo writes one: fetched packages carry a `source`, the
    /// package built from this repository does not.
    const LOCK: &str = "\
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = \"semver\"
version = \"1.0.23\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"
checksum = \"deadbeef\"

[[package]]
name = \"demo\"
version = \"1.2.3\"
dependencies = [
 \"semver\",
]
";

    #[test]
    fn reads_the_version_of_the_package_built_from_this_repository() {
        assert_eq!(
            Format::CargoLock
                .read("Cargo.lock", LOCK, LockScope::Sole)
                .unwrap(),
            v("1.2.3")
        );
    }

    #[test]
    fn cargo_lock_ignores_dependency_versions() {
        // Every fetched package carries a `version` too, and the first one
        // appears before the local package. Only the sourceless entry is ours.
        let out = Format::CargoLock
            .write("Cargo.lock", LOCK, &v("2.0.0"), LockScope::Sole)
            .unwrap();

        assert!(
            out.contains("version = \"1.0.23\""),
            "dependency version must be untouched:\n{out}"
        );
        assert!(out.contains("version = \"2.0.0\""));
        assert!(!out.contains("version = \"1.2.3\""));
    }

    #[test]
    fn cargo_lock_preserves_everything_else() {
        let out = Format::CargoLock
            .write("Cargo.lock", LOCK, &v("2.0.0"), LockScope::Sole)
            .unwrap();

        // The generated-file header and the lock format version both sit above
        // the packages, and rewriting must not disturb either.
        assert!(out.starts_with("# This file is automatically @generated by Cargo."));
        assert!(out.contains("version = 4"));
        assert_eq!(out.matches("[[package]]").count(), 2);
        assert_eq!(out, LOCK.replace("\"1.2.3\"", "\"2.0.0\""));
    }

    /// Cargo's own output for a two-member workspace. See `testdata/README.md`.
    const WORKSPACE: &str = include_str!("testdata/cargo-workspace.lock");

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn a_named_member_is_read_from_a_shared_workspace_lock() {
        let api = names(&["api"]);
        assert_eq!(
            Format::CargoLock
                .read("Cargo.lock", WORKSPACE, LockScope::Named(&api))
                .unwrap(),
            v("1.0.0")
        );

        let web = names(&["web"]);
        assert_eq!(
            Format::CargoLock
                .read("Cargo.lock", WORKSPACE, LockScope::Named(&web))
                .unwrap(),
            v("2.5.0")
        );
    }

    #[test]
    fn writing_one_member_leaves_its_siblings_alone() {
        let api = names(&["api"]);
        let out = Format::CargoLock
            .write("Cargo.lock", WORKSPACE, &v("1.1.0"), LockScope::Named(&api))
            .unwrap();

        assert!(out.contains("name = \"api\"\nversion = \"1.1.0\""), "{out}");
        // The sibling member and the fetched dependency are untouched.
        assert!(out.contains("name = \"web\"\nversion = \"2.5.0\""), "{out}");
        assert!(out.contains("version = \"1.0.28\""), "{out}");
    }

    #[test]
    fn members_moving_together_are_all_written() {
        // A workspace held at one version declares every member in one project,
        // so every matching entry moves.
        let both = names(&["api", "web"]);
        let uniform = WORKSPACE.replace("version = \"2.5.0\"", "version = \"1.0.0\"");

        let out = Format::CargoLock
            .write("Cargo.lock", &uniform, &v("2.0.0"), LockScope::Named(&both))
            .unwrap();

        assert!(out.contains("name = \"api\"\nversion = \"2.0.0\""), "{out}");
        assert!(out.contains("name = \"web\"\nversion = \"2.0.0\""), "{out}");
        assert!(out.contains("version = \"1.0.28\""), "{out}");
    }

    #[test]
    fn members_that_should_move_together_but_disagree_are_refused() {
        // In the fixture the two members sit at different versions, so naming
        // both means asking for one version from two answers.
        let both = names(&["api", "web"]);
        let err = Format::CargoLock
            .read("Cargo.lock", WORKSPACE, LockScope::Named(&both))
            .unwrap_err();

        assert!(
            matches!(err, VersionFileError::InconsistentLock { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn a_lock_naming_none_of_the_projects_packages_is_reported() {
        let absent = names(&["nothing-here"]);
        let err = Format::CargoLock
            .read("Cargo.lock", WORKSPACE, LockScope::Named(&absent))
            .unwrap_err();

        let VersionFileError::MissingField { field, .. } = &err else {
            panic!("got {err:?}");
        };
        assert!(field.contains("nothing-here"), "{field}");
    }

    #[test]
    fn a_manifest_names_the_package_that_pairs_it_with_its_lock_entry() {
        assert_eq!(
            cargo_package_name("[package]\nname = \"api\"\nversion = \"1.0.0\"\n").as_deref(),
            Some("api")
        );
        // A virtual workspace manifest declares no package of its own.
        assert_eq!(
            cargo_package_name("[workspace]\nmembers = [\"crates/*\"]\n"),
            None
        );
    }

    #[test]
    fn a_workspace_lock_is_refused_rather_than_guessed_at() {
        // Two packages built from the repository, so the lock alone cannot say
        // which project a version belongs to.
        let src = LOCK.to_owned() + "\n[[package]]\nname = \"demo-cli\"\nversion = \"1.2.3\"\n";

        let err = Format::CargoLock
            .read("Cargo.lock", &src, LockScope::Sole)
            .unwrap_err();
        let VersionFileError::WorkspaceLock { members, .. } = &err else {
            panic!("expected a workspace refusal, got {err:?}");
        };
        assert_eq!(members, "demo, demo-cli");
    }

    #[test]
    fn a_workspace_lock_is_refused_before_it_is_written() {
        let src = LOCK.to_owned() + "\n[[package]]\nname = \"demo-cli\"\nversion = \"1.2.3\"\n";

        assert!(
            Format::CargoLock
                .write("Cargo.lock", &src, &v("2.0.0"), LockScope::Sole)
                .is_err()
        );
    }

    #[test]
    fn a_lock_with_no_local_package_is_reported() {
        let src = "\
version = 4

[[package]]
name = \"semver\"
version = \"1.0.23\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"
";
        assert!(
            Format::CargoLock
                .read("Cargo.lock", src, LockScope::Sole)
                .is_err()
        );
    }

    #[test]
    fn a_lock_file_is_recognized_by_name() {
        assert_eq!(Format::detect("Cargo.lock"), Some(Format::CargoLock));
    }

    #[test]
    fn cargo_ignores_dependency_versions() {
        // `[dependencies.serde]` puts a bare `version = "..."` at the start of a
        // line, which a line-oriented replace would happily corrupt.
        let src = "\
[package]
name = \"app\"
version = \"1.0.0\"

[dependencies.serde]
version = \"1.0.100\"
";
        let out = Format::CargoToml
            .write("Cargo.toml", src, &v("1.0.1"), LockScope::Sole)
            .unwrap();

        assert!(
            out.contains("version = \"1.0.100\""),
            "dependency pin must be untouched:\n{out}"
        );
        assert!(out.contains("version = \"1.0.1\""));
    }

    #[test]
    fn cargo_workspace_inheritance_is_reported() {
        let src = "[package]\nname = \"app\"\nversion = { workspace = true }\n";
        let err = Format::CargoToml
            .read("Cargo.toml", src, LockScope::Sole)
            .unwrap_err();
        assert!(matches!(err, VersionFileError::InheritedVersion { .. }));
    }

    #[test]
    fn cargo_without_a_package_table_is_reported() {
        let src = "[workspace]\nmembers = [\"a\"]\n";
        let err = Format::CargoToml
            .read("Cargo.toml", src, LockScope::Sole)
            .unwrap_err();
        assert!(matches!(err, VersionFileError::MissingField { .. }));
    }

    #[test]
    fn malformed_toml_is_reported() {
        let err = Format::CargoToml
            .read("Cargo.toml", "[package\nname =", LockScope::Sole)
            .unwrap_err();
        assert!(matches!(err, VersionFileError::Malformed { .. }));
    }

    #[test]
    fn non_semver_contents_are_reported() {
        let err = Format::PlainText
            .read("VERSION", "not-a-version", LockScope::Sole)
            .unwrap_err();
        assert!(matches!(err, VersionFileError::InvalidVersion { .. }));
    }

    #[test]
    fn round_trips_through_read_after_write() {
        let cases: &[(Format, &str, &str)] = &[
            (Format::PlainText, "VERSION", "1.0.0\n"),
            (
                Format::PackageJson,
                "package.json",
                r#"{"version":"1.0.0"}"#,
            ),
            (
                Format::CargoToml,
                "Cargo.toml",
                "[package]\nversion = \"1.0.0\"\n",
            ),
        ];
        let next = v("2.3.4-rc.1");
        for (format, name, src) in cases {
            let written = format.write(name, src, &next, LockScope::Sole).unwrap();
            assert_eq!(
                format.read(name, &written, LockScope::Sole).unwrap(),
                next,
                "{name}"
            );
        }
    }
}
