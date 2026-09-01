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

/// A file format vump knows how to read a version from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// npm manifest; the version lives at the top-level `version` key.
    PackageJson,
    /// Cargo manifest; the version lives at `[package].version`.
    CargoToml,
    /// A file whose entire contents are the version.
    PlainText,
}

/// Why a version could not be read from or written to a file's contents.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VersionFileError {
    /// The filename is not one vump recognizes.
    #[error("unsupported file {name:?}; expected package.json, Cargo.toml, or VERSION")]
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
            Self::PackageJson => "JSON",
            Self::CargoToml => "TOML",
            Self::PlainText => "plain text",
        }
    }

    /// Describes where the version lives, used in diagnostics.
    #[must_use]
    pub fn field_description(self) -> &'static str {
        match self {
            Self::PackageJson => "\"version\"",
            Self::CargoToml => "[package].version",
            Self::PlainText => "version",
        }
    }

    /// Reads the version recorded in `contents`.
    ///
    /// `file` is used only to label diagnostics.
    ///
    /// # Errors
    ///
    /// Returns a [`VersionFileError`] when the contents cannot be parsed, carry
    /// no version, or carry one that is not valid semver.
    pub fn read(self, file: &str, contents: &str) -> Result<Version, VersionFileError> {
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
    let bytes = contents.as_bytes();
    let mut i = 0;
    let mut depth: i32 = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'{' | b'[' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' => {
                depth -= 1;
                i += 1;
            }
            b'"' => {
                let span = string_span(bytes, i)?;
                // Position just past the closing quote.
                let after = span.end + 1;
                let next = skip_whitespace(bytes, after);

                // A string followed by ':' is a key. Only root-object keys are
                // candidates, and the root object puts its keys at depth 1.
                if next < bytes.len() && bytes[next] == b':' && depth == 1 {
                    let value_start = skip_whitespace(bytes, next + 1);
                    if &contents[span.clone()] == "version" {
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
        assert_eq!(f.read("VERSION", "1.2.3\n").unwrap(), v("1.2.3"));
        assert_eq!(
            f.read("VERSION", "  1.2.3-rc.1  ").unwrap(),
            v("1.2.3-rc.1")
        );
    }

    #[test]
    fn writes_plain_text_with_trailing_newline() {
        let out = Format::PlainText
            .write("VERSION", "1.2.3\n", &v("1.3.0"))
            .unwrap();
        assert_eq!(out, "1.3.0\n");
    }

    #[test]
    fn reads_package_json() {
        let src = r#"{"name":"app","version":"4.5.6"}"#;
        assert_eq!(
            Format::PackageJson.read("package.json", src).unwrap(),
            v("4.5.6")
        );
    }

    #[test]
    fn package_json_rewrite_preserves_everything_else() {
        // Deliberately unusual formatting: tabs, odd spacing, key order.
        let src = "{\n\t\"name\"  :  \"app\",\n\t\"version\":\t\"1.0.0\",\n\t\"scripts\": {\n\t\t\"build\": \"tsc\"\n\t}\n}\n";
        let out = Format::PackageJson
            .write("package.json", src, &v("1.0.1"))
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
        let read = Format::PackageJson.read("package.json", src).unwrap();
        assert_eq!(
            read,
            v("1.0.0"),
            "must read the root version, not a nested one"
        );

        let out = Format::PackageJson
            .write("package.json", src, &v("2.0.0"))
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
            Format::PackageJson.read("package.json", src).unwrap(),
            v("3.0.0")
        );
    }

    #[test]
    fn package_json_without_a_version_is_reported() {
        let err = Format::PackageJson
            .read("package.json", r#"{"name":"app"}"#)
            .unwrap_err();
        assert!(matches!(err, VersionFileError::MissingField { .. }));
    }

    #[test]
    fn reads_cargo_toml() {
        let src = "[package]\nname = \"app\"\nversion = \"0.4.2\"\n";
        assert_eq!(
            Format::CargoToml.read("Cargo.toml", src).unwrap(),
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
            .write("Cargo.toml", src, &v("0.5.0"))
            .unwrap();

        assert!(out.contains("# Project manifest"));
        assert!(
            out.contains("# bumped by vump"),
            "trailing comment lost:\n{out}"
        );
        assert!(out.contains("serde = \"1\""));
        assert_eq!(out, src.replace("0.4.2", "0.5.0"));
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
            .write("Cargo.toml", src, &v("1.0.1"))
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
        let err = Format::CargoToml.read("Cargo.toml", src).unwrap_err();
        assert!(matches!(err, VersionFileError::InheritedVersion { .. }));
    }

    #[test]
    fn cargo_without_a_package_table_is_reported() {
        let src = "[workspace]\nmembers = [\"a\"]\n";
        let err = Format::CargoToml.read("Cargo.toml", src).unwrap_err();
        assert!(matches!(err, VersionFileError::MissingField { .. }));
    }

    #[test]
    fn malformed_toml_is_reported() {
        let err = Format::CargoToml
            .read("Cargo.toml", "[package\nname =")
            .unwrap_err();
        assert!(matches!(err, VersionFileError::Malformed { .. }));
    }

    #[test]
    fn non_semver_contents_are_reported() {
        let err = Format::PlainText
            .read("VERSION", "not-a-version")
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
            let written = format.write(name, src, &next).unwrap();
            assert_eq!(format.read(name, &written).unwrap(), next, "{name}");
        }
    }
}
